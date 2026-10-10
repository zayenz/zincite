use std::path::PathBuf;
use zincite_lint::*;

const CORE: &str = concat!(
    "function int: '+'(int:a,int:b); function int: '-'(int:a,int:b); function set of int: '..'(int:a,int:b);\n",
    "function bool: '<'(int:a,int:b); function bool: 'in'(int:a,set of int:b);\n",
    "function var bool: '='(any $T:a,any $T:b); function var bool: '->'(var bool:a,var bool:b); function var bool: '\\/'(var bool:a,var bool:b);\n",
    "function var bool: forall(array[int] of var bool:a); function set of $I: index_set(array[$I] of $T:a);\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext, AnalysisResult) {
    let dir = std::env::temp_dir().join(format!(
        "zincite-index-mismatch-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), CORE).unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            include_dirs: vec![],
            stdlib_dir: Some(dir.join("library")),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("index-set-mismatch").unwrap(),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    (dir, context, result)
}
fn spelling<'a>(context: &'a ModelContext, finding: &FileFinding) -> &'a str {
    let file = context
        .files
        .iter()
        .find(|f| f.path == finding.location.path)
        .unwrap();
    file.parsed.source()[finding.location.range.start - file.byte_offset
        ..finding.location.range.end - file.byte_offset]
        .trim()
}
#[test]
fn ranges_offsets_and_dimensions_report_actual_membership_not_length() {
    let source = concat!(
        "array[1..3] of int:a; array[0..2] of int:z; array[1..3,0..2] of int:grid;\n",
        "constraint forall(i in 0..2)(a[i]=0); constraint forall(j in 1..3)(a[j+1]=0);\n",
        "constraint forall(k in 0..2)(a[k+1]=0); constraint forall(q in 0..2)(z[q]=0);\n",
        "int:outside=a[4]; array[0..2] of int:bad_slice=a[0..2]; array[1..2] of int:valid_subset=a[1..2];\n",
        "constraint forall(r in 1..3,c in 1..3)(grid[r,c]=0);\n",
        "array[2..2] of int:center; constraint forall(s in {1,3})(center[s]=0); constraint forall(t in {1,3})(a[t]=0); solve satisfy;\n",
    );
    let (dir, context, result) = model("spaces", source);
    assert_eq!(result.status(), 1);
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{result:?}"
    );
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        [
            "a[i]",
            "a[j+1]",
            "a[4]",
            "a[0..2]",
            "grid[r,c]",
            "center[s]"
        ]
    );
    for f in &result.findings {
        assert_eq!(f.rule, Rule::IndexSetMismatch);
        assert!(
            f.message.contains("declared at") && f.message.contains("root.mzn:"),
            "{}",
            f.message
        );
        assert!(!f.message.contains("runtime error"));
    }
    assert!(
        result.findings[0]
            .message
            .contains("some candidate indices")
    );
    assert!(result.findings[1].message.contains("2..4"));
    assert!(result.findings[2].message.contains("wholly outside"));
    assert!(result.findings[3].message.contains("0..2"));
    assert!(
        result.findings[5].message.contains("{1, 3}")
            && result.findings[5].message.contains("wholly outside")
    );
    assert!(
        result.findings[4]
            .message
            .contains("array 'grid' dimension 2 (0..2")
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn local_guards_keep_safe_neighbors_and_unknown_restrictions_quiet() {
    let source = concat!(
        "array[1..3] of int:a; bool:keep;\n",
        "constraint forall(i in 1..3)(a[i+1]=0);\n",
        "constraint forall(j in 1..3)(if j<3 then a[j+1]=0 else true endif);\n",
        "constraint forall(k in 1..3 where k<3)(a[k+1]=0);\n",
        "constraint forall(q in 1..3)(if q+1 in index_set(a) then a[q+1]=0 else true endif);\n",
        "constraint forall(e in 1..0)(a[e+1]=0); int:inactive=if false then a[9] else 0 endif;\n",
        "constraint forall(u in 1..3 where keep)(a[u+1]=0);\n",
        "constraint forall(v in 1..3)(if keep then a[v+1]=0 else true endif);\n",
        "constraint forall(w in 1..3)(w<3 -> a[w+1]=0); bool:relational=a[4]=0; solve satisfy;\n",
    );
    let (dir, context, result) = model("guards", source);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        ["a[i+1]", "a[w+1]", "a[4]"]
    );
    assert!(
        result
            .findings
            .iter()
            .all(|f| !f.message.contains("inconsistent") && !f.message.contains("runtime error"))
    );
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let defs = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let numeric = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &defs);
    let guarded = resolve_guarded_facts(&context, &bindings, &calls, &inst, &domains, &numeric);
    let file = context
        .files
        .iter()
        .position(|f| f.path == dir.join("root.mzn"))
        .unwrap();
    let protected = guarded
        .obligations
        .iter()
        .find(|o| o.file == file && &source[o.operation.range.clone()] == "a[j+1]")
        .unwrap();
    assert_eq!(protected.invariant, GuardedOutcome::Unknown);
    assert_eq!(protected.outcome, GuardedOutcome::Proven);
    assert!(
        matches!(&protected.kind,GuardObligationKind::Index{selection:Some(s),..} if s.exact && matches!(s.domain,Domain::Range{lower:NumericBound::Integer(2),upper:NumericBound::Integer(3)}))
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn enum_identity_unknown_data_and_unsupported_slices_have_distinct_outcomes() {
    let source = concat!(
        "enum X={A,B,C}; enum Y={D,E,F}; array[X] of int:a; array[0..2] of int:z;\n",
        "constraint forall(x in X)(a[x]=0); constraint forall(y in Y)(a[y]=0); constraint forall(q in 0..2)(z[q]=0);\n",
        "X:good; Y:other; int:valid_scalar=a[good]; int:invalid_scalar=a[other];\n",
        "int:N=3; int:K=3; set of int:P={1,2,3}; set of int:Q={1,2,3}; array[1..N] of int:b; array[P] of int:c;\n",
        "constraint forall(n in 1..K)(b[n]=0); constraint forall(p in Q)(c[p]=0); solve satisfy;\n",
    );
    let (dir, context, result) = model("identities", source);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        ["a[y]", "a[other]"]
    );
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.message.contains("incompatible enum index type"))
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    std::fs::remove_dir_all(dir).unwrap();
    let source = concat!(
        "enum X={A,B}; array[X,1..2] of int:grid;\n",
        "array[X,1..2] of int:whole=grid[..,..]; array[1..2] of int:row=grid[A,..]; array[X] of int:column=grid[..,1]; solve satisfy;\n",
    );
    let (dir, context, result) = model("full-axes", source);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert!(
        result.findings.is_empty() && result.limitations.is_empty(),
        "{result:?}"
    );
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let axis = bindings
        .declarations
        .iter()
        .find(|d| d.name == "X")
        .unwrap()
        .id;
    for (access, expected) in [
        ("grid[..,..]", vec![TypeKind::Enum(axis), TypeKind::Int]),
        ("grid[A,..]", vec![TypeKind::Int]),
        ("grid[..,1]", vec![TypeKind::Enum(axis)]),
    ] {
        let ty = &calls
            .expressions
            .iter()
            .find(|e| {
                e.location.path == dir.join("root.mzn")
                    && &source[e.location.range.clone()] == access
            })
            .unwrap()
            .ty;
        assert_eq!(ty.instantiation, Instantiation::Parameter);
        assert!(!ty.optional);
        let TypeKind::Array { indices, element } = &ty.kind else {
            panic!("{access}: {ty:?}");
        };
        assert_eq!(
            indices.iter().map(|i| i.kind.clone()).collect::<Vec<_>>(),
            expected
        );
        assert!(
            indices
                .iter()
                .all(|i| i.instantiation == Instantiation::Parameter && !i.optional)
        );
        assert_eq!(element.kind, TypeKind::Int);
        assert_eq!(element.instantiation, Instantiation::Parameter);
        assert!(!element.optional);
    }
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "decision-slice",
        "enum X={A,B}; array[X,1..2] of int:grid; var int:k; array[X] of var int:slice=grid[..,k]; solve satisfy;",
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(result.findings.is_empty());
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("type is unavailable")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "explicit-range",
        "array[1..2,1..2] of int:grid; array[int,int] of int:slice=grid[..,1..2]; solve satisfy;",
    );
    assert_eq!(result.status(), 0);
    assert!(result.findings.is_empty());
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert!(result.limitations.is_empty(), "{result:?}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn initialized_boundary_unions_keep_unproved_indices_quiet() {
    let source = r#"% Symbolic parameters remain unassigned during lint analysis.
int: row_count;
int: column_count;
int: step_count;

set of int: Rows = 0..row_count - 1;
set of int: Columns = 0..column_count - 1;
set of int: Cells = 0..row_count * column_count - 1;
set of int: Outside = -2..-1;
set of int: Places = Cells union Outside;
set of int: Steps = 0..step_count - 1;

array[Rows, Columns] of Cells: grid = array2d(Rows, Columns,
    [r * column_count + c | r in Rows, c in Columns]
);
set of Cells: Border = {grid[r, 0] | r in Rows}
    union {grid[r, column_count - 1] | r in Rows};

array[Steps, Places] of var 0..1: levels;
array[Steps, Places] of var 0..1: fixed_levels;

% This selects a boundary subset, not every place in the declared array.
constraint :: "Boundary cells remain empty"
forall (t in Steps, p in Border) (
    levels[t, p] = 0
);

% A same-axis control must keep its whole-array constant-variable advice.
constraint :: "Every fixed level is empty"
forall (t in Steps, p in Places) (
    fixed_levels[t, p] = 0
);

solve satisfy;
"#;
    let dir = std::env::temp_dir().join(format!(
        "zincite-index-mismatch-boundary-union-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        concat!(
            "function set of $T: 'union'(set of $T: x,set of $T: y);\n",
            "function set of $$E: '..'($$E: x,$$E: y);\n",
            "function int: '-'(int: x,int: y); function int: '-'(int: x);\n",
            "function int: '+'(int: x,int: y); function int: '*'(int: x,int: y);\n",
            "function var bool: '='(any $T: x,any $T: y);\n",
            "function var bool: forall(array[$T] of var bool: body);\n",
            "function array[$$E,$$F] of any $V: array2d(set of $$E: rows,set of $$F: columns,array[$U] of any $V: values);\n",
        ),
    ).unwrap();
    let root = dir.join("root.mzn");
    std::fs::write(&root, source).unwrap();
    let options = ModelOptions {
        include_dirs: Vec::new(),
        stdlib_dir: Some(dir.join("library")),
    };
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let parameter_int = |ty: &TypeInst| {
        !ty.optional && ty.instantiation == Instantiation::Parameter && ty.kind == TypeKind::Int
    };
    let parameter_set_int = |ty: &TypeInst| {
        !ty.optional
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element) if parameter_int(element))
    };
    let union_calls: Vec<_> = calls
        .calls
        .iter()
        .filter(|call| call.location.path == root && call.name == "union")
        .collect();
    assert_eq!(union_calls.len(), 2);
    for call in union_calls {
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &call.outcome
        else {
            panic!("union prerequisite: {:?}", call.outcome);
        };
        let selected = &bindings.declarations[declaration.0];
        assert_eq!(selected.name, "union");
        assert_eq!(
            context.files[selected.file].kind,
            SourceKind::StandardLibrary
        );
        assert!(context.files[selected.file].implicit);
        assert_eq!(parameters.len(), 2);
        assert!(parameters.iter().all(parameter_set_int) && parameter_set_int(return_type));
    }
    let border = bindings
        .declarations
        .iter()
        .find(|d| d.location.path == root && d.top_level && d.name == "Border")
        .unwrap();
    assert_eq!(border.role, DeclarationRole::Value);
    let typed_border = &calls.declarations[border.id.0];
    assert_eq!(typed_border.declaration, border.id);
    assert!(parameter_set_int(&typed_border.ty));
    for name in ["Cells", "Outside"] {
        let id = bindings
            .declarations
            .iter()
            .find(|d| d.location.path == root && d.top_level && d.name == name)
            .unwrap()
            .id;
        assert_eq!(calls.declarations[id.0].declaration, id);
        assert!(parameter_set_int(&calls.declarations[id.0].ty));
    }
    let (root_file, owning_source) = context
        .files
        .iter()
        .enumerate()
        .find(|(_, file)| file.path == root)
        .unwrap();
    for operand in [
        "{grid[r, 0] | r in Rows}",
        "{grid[r, column_count - 1] | r in Rows}",
    ] {
        let mut nodes = vec![owning_source.parsed.tree()];
        let mut matches = Vec::new();
        while let Some(node) = nodes.pop() {
            if node.kind() == zincite_syntax::NodeKind::SetComprehension
                && owning_source.parsed.source()[node.range()].trim() == operand
            {
                matches.push(node);
            }
            nodes.extend(node.child_nodes());
        }
        assert_eq!(
            matches.len(),
            1,
            "expected one CST set comprehension for {operand:?}"
        );
        let location = owning_source.location(matches[0].range());
        let actual = calls
            .expressions
            .iter()
            .find(|expression| {
                expression.file == root_file && expression.location.range == location.range
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing expression fact for {operand:?} at {location:?}; root facts: {:?}",
                    calls
                        .expressions
                        .iter()
                        .filter(|expression| expression.file == root_file)
                        .map(|expression| (&expression.location.range, &expression.ty))
                        .collect::<Vec<_>>()
                )
            });
        assert!(parameter_set_int(&actual.ty), "{actual:?}");
    }
    let border_use = source.find("p in Border").unwrap() + "p in ".len();
    let reference = bindings
        .references
        .iter()
        .find(|reference| {
            reference.location.path == root
                && reference.location.range.start == border_use
                && reference.kind == ReferenceKind::Value
        })
        .unwrap();
    assert_eq!(reference.resolution, BindingResolution::Resolved(border.id));
    assert!(
        calls
            .calls
            .iter()
            .filter(|call| call.location.path == root)
            .all(|call| {
                matches!(
                    call.outcome,
                    CallOutcome::Resolved { .. } | CallOutcome::Intrinsic { .. }
                )
            }),
        "{:?}",
        calls.calls
    );

    // A supported symbolic source remains quiet without a membership warning.
    // Data used for compiler acceptance is deliberately not loaded.
    let selected = LintOptions::from_selection("index-set-mismatch").unwrap();
    let result = analyze_model(&context, &selected);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.rules[0].outcome,
        RuleOutcome::Completed,
        "{result:?}"
    );
    assert!(
        result.findings.is_empty() && result.limitations.is_empty(),
        "{result:?}"
    );
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);

    // Symbolic iteration cannot conceal an independently closed bad head.
    let hazardous = source.replace("grid[r, 0]", "9223372036854775807 + 1");
    assert_ne!(hazardous, source);
    std::fs::write(&root, &hazardous).unwrap();
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &selected);
    assert!(
        result.errors.is_empty() && result.findings.is_empty(),
        "{result:?}"
    );
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
        "{result:?}"
    );
    assert!(
        result.limitations.iter().any(|limit| {
            limit.location.path == root
                && &hazardous[limit.location.range.clone()] == "levels[t, p]"
                && limit.message.contains("integer arithmetic overflow")
        }),
        "{:?}",
        result.limitations
    );
    assert_eq!(std::fs::read_to_string(&root).unwrap(), hazardous);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn symbolic_extremum_offsets_keep_membership_unproved() {
    use zincite_syntax::NodeKind;

    let source = r#"int: width;
int: height;
int: step_count;
set of int: Columns = 0..width-1;
set of int: Rows = 0..height-1;
set of int: Cells = 0..width*height-1;
set of int: Times = 0..step_count-1;
set of int: OtherTimes = 0..step_count-2;
set of int: Outside = {-2, -1};
set of int: World = Cells union Outside;
enum Action = { WAIT, MOVE };
array[Columns, Rows] of Cells: cell_id =
    array2d(Columns, Rows, [y*width+x | x in Columns, y in Rows]);
array[Cells] of set of Cells: neighbours = array1d(Cells, [
    (if x > 0 then {cell_id[x-1,y]} else {} endif)
    union (if x < width-1 then {cell_id[x+1,y]} else {} endif)
    union (if y > 0 then {cell_id[x,y-1]} else {} endif)
    union (if y < height-1 then {cell_id[x,y+1]} else {} endif)
    | y in Rows, x in Columns
]);
set of Cells: Border =
    {cell_id[x,0] | x in Columns} union
    {cell_id[x,height-1] | x in Columns} union
    {cell_id[0,y] | y in Rows} union
    {cell_id[width-1,y] | y in Rows};
array[Times, World] of var Action: action;
array[Times, Cells] of var Cells: block_cell;
array[Times, World] of var World: next_cell;
constraint forall(t in min(Times)+1..max(Times)-1, i in Cells)(
    sum(j in neighbours[i])(
        bool2int(action[t+1,j] = MOVE /\ block_cell[t+1,j] = i)
    ) <= 1
);
constraint forall(t in min(Times)+1..max(Times))(
    sum(i in Border)(
        bool2int(action[t-1,i] = WAIT /\ next_cell[t-1,i] = i)
    ) <= 1
);
solve satisfy;
"#;
    let dir = std::env::temp_dir().join(format!(
        "zincite-index-mismatch-extremum-offset-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        concat!(
            "function set of int: '..'(int: left,int: right);\n",
            "function int: '-'(int: left,int: right); function int: '-'(int: value);\n",
            "function int: '+'(int: left,int: right); function int: '*'(int: left,int: right);\n",
            "function bool: '>'(int: left,int: right); function bool: '<'(int: left,int: right);\n",
            "function set of int: 'union'(set of int: left,set of int: right);\n",
            "function array[$$E] of any $V: array1d(set of $$E: S,array[$U] of any $V: x);\n",
            "function array[$$E,$$F] of any $V: array2d(set of $$E: S1,set of $$F: S2,array[$U] of any $V: x);\n",
            "function var bool: '='(any $T: left,any $T: right);\n",
            "function var bool: '/\\'(var bool: left,var bool: right);\n",
            "function var bool: '<='(var int: left,var int: right);\n",
            "function var bool: forall(array[$T] of var bool: body);\n",
            "function var int: sum(array[$T] of var int: body); function var int: bool2int(var bool: body);\n",
            "function $$E: min(set of $$E: s); function $$E: max(set of $$E: s);\n",
        ),
    ).unwrap();
    let root = dir.join("root.mzn");
    std::fs::write(&root, source).unwrap();
    let options = ModelOptions {
        include_dirs: Vec::new(),
        stdlib_dir: Some(dir.join("library")),
    };
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let file = context
        .files
        .iter()
        .position(|file| file.path == root)
        .unwrap();
    let written = &context.files[file];
    let mut accesses = Vec::new();
    let mut pending = vec![written.parsed.tree()];
    while let Some(node) = pending.pop() {
        if node.kind() == NodeKind::ArrayAccessExpression
            && [
                "action[t+1,j]",
                "block_cell[t+1,j]",
                "action[t-1,i]",
                "next_cell[t-1,i]",
            ]
            .contains(&written.parsed.source()[node.range()].trim())
        {
            accesses.push(written.location(node.range()).range);
        }
        pending.extend(node.child_nodes());
    }
    assert_eq!(accesses.len(), 4);
    let selected = LintOptions::from_selection("index-set-mismatch").unwrap();
    let result = analyze_model(&context, &selected);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        result
            .findings
            .iter()
            .all(|finding| !accesses.contains(&finding.location.range)),
        "{result:?}"
    );

    // The raw fact view checks only that no membership or traversal was proved.
    // The call-aware rule result may retain the separate neighbours-source gap.
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let numeric = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
    let guarded = resolve_guarded_facts(&context, &bindings, &calls, &inst, &domains, &numeric);
    let offsets: Vec<_> = guarded
        .obligations
        .iter()
        .filter(|obligation| {
            obligation.file == file
                && accesses.contains(&obligation.operation.range)
                && matches!(
                    obligation.kind,
                    GuardObligationKind::Index { dimension: 1, .. }
                )
        })
        .collect();
    assert_eq!(offsets.len(), 4);
    for obligation in offsets {
        assert_eq!(obligation.invariant, GuardedOutcome::Unknown);
        assert_eq!(obligation.outcome, GuardedOutcome::Unknown);
        let GuardObligationKind::Index {
            selection: Some(selection),
            ..
        } = &obligation.kind
        else {
            panic!("{obligation:?}");
        };
        assert!(
            !selection.exact && selection.array_dimension.is_none(),
            "{selection:?}"
        );
        assert_eq!(selection.domain, Domain::Unknown, "{obligation:?}");
    }
    assert!(
        !result.limitations.iter().any(|limit| {
            accesses.contains(&limit.location.range)
                && limit.message.contains("offset of an opaque index set")
        }),
        "{result:?}"
    );
    assert!(!definitions.definitions.iter().any(|definition| {
        definition.coverage == DefinitionCoverage::WholeArray
            && ["action", "block_cell", "next_cell"]
                .contains(&bindings.declarations[definition.target.0].name.as_str())
    }));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);

    // A closed selector error stays explicit even inside a symbolic iteration.
    let hazardous = source.replace("t+1", "t+(9223372036854775807 + 1)");
    assert_ne!(hazardous, source);
    std::fs::write(&root, &hazardous).unwrap();
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &selected);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
        "{result:?}"
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|limit| limit.message.contains("integer arithmetic overflow")),
        "{result:?}"
    );
    assert_eq!(std::fs::read_to_string(&root).unwrap(), hazardous);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn selected_parameter_sets_keep_unknown_membership_and_lazy_error_vetoes() {
    let source = r#"int: width;
int: height;
set of int: Columns = 0..width-1;
set of int: Rows = 0..height-1;
set of int: Cells = 0..width*height-1;
array[Columns, Rows] of Cells: cell_id =
    array2d(Columns, Rows, [y*width+x | x in Columns, y in Rows]);
array[Cells] of set of Cells: neighbours = array1d(Cells, [
    (if x > 0 then {cell_id[x-1,y]} else {} endif)
    union (if x < width-1 then {cell_id[x+1,y]} else {} endif)
    union (if y > 0 then {cell_id[x,y-1]} else {} endif)
    union (if y < height-1 then {cell_id[x,y+1]} else {} endif)
    | y in Rows, x in Columns
]);
array[Cells] of var 0..1: load;
constraint forall(i in Cells)(sum(j in neighbours[i])(load[j]) <= 1);
solve satisfy;
"#;
    let dir = std::env::temp_dir().join(format!("zincite-selected-set-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), format!("{CORE}{}", concat!(
        "function int: '*'(int:a,int:b); function int: 'div'(int:a,int:b);\n",
        "function bool: '>'(int:a,int:b); function var bool: '<='(var int:a,var int:b);\n",
        "function set of int: 'union'(set of int:a,set of int:b);\n",
        "function array[$$E] of any $V: array1d(set of $$E:S,array[$U] of any $V:x);\n",
        "function array[$$E,$$F] of any $V: array2d(set of $$E:S1,set of $$F:S2,array[$U] of any $V:x);\n",
        "function var int: sum(array[$T] of var int:x);\n",
    ))).unwrap();
    let root = dir.join("root.mzn");
    let options = ModelOptions {
        include_dirs: vec![],
        stdlib_dir: Some(dir.join("library")),
    };
    let selected = LintOptions::from_selection("index-set-mismatch,search-coverage").unwrap();
    let inspect = |text: &str| {
        std::fs::write(&root, text).unwrap();
        let context = load_model(&root, &options);
        assert!(context.errors.is_empty(), "{:?}", context.errors);
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
        let numeric =
            resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
        let guarded = resolve_guarded_facts(&context, &bindings, &calls, &inst, &domains, &numeric);
        let result = analyze_model(&context, &selected);
        assert!(result.errors.is_empty(), "{result:?}");
        assert_eq!(std::fs::read_to_string(&root).unwrap(), text);
        (context, guarded, result)
    };
    let inactive = source.replace("if x > 0 then {cell_id[x-1,y]}", "if false then {1 div 0}");
    for text in [source, inactive.as_str()] {
        let (context, guarded, result) = inspect(text);
        assert_eq!(
            result
                .rules
                .iter()
                .find(|r| r.rule.id() == "search-coverage")
                .unwrap()
                .outcome,
            RuleOutcome::Completed,
            "{result:?}"
        );
        let target = guarded
            .obligations
            .iter()
            .find(|o| {
                context.files[o.file].path == root
                    && context.files[o.file].parsed.source()[o.operation.range.clone()].trim()
                        == "load[j]"
            })
            .unwrap();
        assert_eq!(target.invariant, GuardedOutcome::Unknown);
        assert_eq!(target.outcome, GuardedOutcome::Unknown);
        let GuardObligationKind::Index {
            selection: Some(selection),
            ..
        } = &target.kind
        else {
            panic!("{target:?}");
        };
        assert_eq!(selection.domain, Domain::Unknown);
        assert!(!selection.exact && selection.array_dimension.is_none());
        assert!(
            !result
                .findings
                .iter()
                .any(|f| f.location.range == target.operation.range),
            "{result:?}"
        );
        assert!(
            !result
                .limitations
                .iter()
                .any(|l| l.location.range == target.operation.range),
            "{result:?}"
        );
    }
    let overflow = source.replace("neighbours[i]", "neighbours[i+(9223372036854775807 + 1)]");
    let (_, _, result) = inspect(&overflow);
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("integer arithmetic overflow")),
        "{result:?}"
    );
    let active = source.replace("if x > 0 then {cell_id[x-1,y]}", "if true then {1 div 0}");
    let (context, guarded, result) = inspect(&active);
    assert!(
        matches!(
            result
                .rules
                .iter()
                .find(|r| r.rule.id() == "search-coverage")
                .unwrap()
                .outcome,
            RuleOutcome::Limited { .. }
        ),
        "{result:?}"
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.starts_with("search-coverage:")
                && l.message.contains("division by zero")),
        "{result:?}"
    );
    let error = guarded
        .obligations
        .iter()
        .find(|o| {
            context.files[o.file].path == root
                && context.files[o.file].parsed.source()[o.operation.range.clone()].trim()
                    == "1 div 0"
        })
        .unwrap();
    assert_eq!(error.invariant, GuardedOutcome::Refuted);
    assert_eq!(error.outcome, GuardedOutcome::Refuted);
    assert_eq!(error.context.activation, GuardActivation::Conditional);
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("division by zero")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

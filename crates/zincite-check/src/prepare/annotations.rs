use super::*;

#[derive(Clone, Copy)]
enum SearchRole {
    Search,
    WarmStart,
    VariableChoice,
    ValueChoice,
    FloatValueChoice,
    Exploration,
}

impl Lowerer<'_> {
    pub(super) fn metadata(
        &mut self,
        annotations: &[&SyntaxNode],
        context: AnnotationContext,
    ) -> Result<Metadata> {
        let mut metadata = Metadata::default();
        for annotation in annotations {
            let expression = annotation
                .child_nodes()
                .next()
                .ok_or_else(|| self.source.error(annotation, "malformed annotation"))?;
            if self
                .source
                .tokens(expression)
                .next()
                .is_some_and(|(kind, _)| kind == TokenKind::StringLiteral)
            {
                if !matches!(context, AnnotationContext::Constraint) {
                    return Err(self
                        .source
                        .error(expression, "string labels belong to constraints"));
                }
                self.set_label(&mut metadata, self.source.string(expression)?, expression)?;
                continue;
            }
            let name = self.source.name(expression)?;
            if !name.as_bytes()[0].is_ascii_alphabetic() {
                return Err(self
                    .source
                    .error(expression, "annotation names must start with a letter"));
            }
            let args: Vec<_> = expression.child_nodes().collect();
            let call = expression.kind() == NodeKind::CallExpression;
            let flag = !call && expression.kind() == NodeKind::Expression && args.is_empty();
            match (name.as_str(), context) {
                ("checker_value", AnnotationContext::Value { is_var: true, .. })
                | ("checker_constraint" | "checker_value", AnnotationContext::Constraint)
                    if call && args.len() == 2 =>
                {
                    let application = Application {
                        id: self.source.string(args[0])?,
                        entities: self.source.integer_array(args[1])?,
                    };
                    let role = if name == "checker_value"
                        && matches!(context, AnnotationContext::Constraint)
                    {
                        &mut metadata.value_application
                    } else {
                        &mut metadata.application
                    };
                    if role
                        .as_ref()
                        .is_some_and(|previous| previous != &application)
                    {
                        return Err(self
                            .source
                            .error(expression, "conflicting application annotations"));
                    }
                    *role = Some(application);
                }
                (
                    "mzn_path",
                    AnnotationContext::Value { .. }
                    | AnnotationContext::Constraint
                    | AnnotationContext::Operand,
                ) if call && args.len() == 1 => {
                    let path = if self
                        .source
                        .tokens(args[0])
                        .next()
                        .is_some_and(|(kind, _)| kind == TokenKind::StringLiteral)
                    {
                        self.source.string(args[0])?
                    } else {
                        self.constant_integer(args[0])?.to_string()
                    };
                    metadata.paths.push(path);
                }
                ("mzn_constraint_name" | "constraint_name", AnnotationContext::Constraint)
                    if call && args.len() == 1 =>
                {
                    self.set_label(&mut metadata, self.source.string(args[0])?, expression)?;
                }
                (
                    "mzn_expression_name" | "expression_name" | "doc_comment",
                    AnnotationContext::Value { .. } | AnnotationContext::Constraint,
                ) if call && args.len() == 1 => {
                    self.source.string(args[0])?;
                }
                (
                    "is_defined_var",
                    AnnotationContext::Value { is_var: true, .. } | AnnotationContext::Constraint,
                ) if flag => metadata.is_defined = true,
                ("defines_var", AnnotationContext::Constraint) if call && args.len() == 1 => {
                    let target = match args[0].kind() {
                        NodeKind::Expression => DefinitionTarget::Name(self.source.name(args[0])?),
                        NodeKind::ArrayLiteral => {
                            let names = args[0]
                                .child_nodes()
                                .map(|node| {
                                    if node.kind() != NodeKind::Expression {
                                        return Err(self.source.error(
                                            node,
                                            "defines_var array needs scalar variable names",
                                        ));
                                    }
                                    self.source.name(node)
                                })
                                .collect::<Result<Vec<_>>>()?;
                            DefinitionTarget::Elements(names)
                        }
                        _ => {
                            return Err(self
                                .source
                                .error(args[0], "defines_var needs a name or scalar-name array"));
                        }
                    };
                    if metadata
                        .defines
                        .as_ref()
                        .is_some_and(|previous| previous != &target)
                    {
                        return Err(self
                            .source
                            .error(expression, "conflicting defines_var annotations"));
                    }
                    metadata.defines = Some(target);
                }
                (
                    "output_var",
                    AnnotationContext::Value {
                        is_array: false,
                        is_var: true,
                        ..
                    },
                ) if flag => metadata.output = true,
                ("output_var", AnnotationContext::Constraint) if flag => metadata.output = true,
                (
                    "output_array",
                    AnnotationContext::Value {
                        is_array: true,
                        is_var: true,
                        length: Some(length),
                    },
                ) if call && args.len() == 1 => {
                    self.output_dimensions(args[0], length)?;
                    metadata.output = true;
                }
                (
                    "computed_domain" | "var_is_introduced" | "mzn_check_var" | "add_to_output"
                    | "no_output",
                    AnnotationContext::Value { is_var: true, .. },
                ) if flag => {}
                ("mzn_check_enum_var", AnnotationContext::Value { is_var: true, .. })
                    if call && args.len() == 1 =>
                {
                    self.fixed_array(args[0], ValueType::IntegerSet)?;
                }
                (
                    "domain"
                    | "bounds"
                    | "domain_propagation"
                    | "bounds_propagation"
                    | "value_propagation"
                    | "gecode_basic_propagation"
                    | "gecode_advanced_propagation"
                    | "gecode_full_propagation"
                    | "gecode_decomposed_propagation"
                    | "domain_change_constraint",
                    AnnotationContext::Constraint,
                ) if flag => {}
                ("empty_annotation", _) if flag => {}
                (
                    "promise_total"
                    | "maybe_partial"
                    | "no_cse"
                    | "ctx_root"
                    | "ctx_pos"
                    | "ctx_neg"
                    | "ctx_mix"
                    | "mzn_rhs_from_assignment",
                    AnnotationContext::Value { .. } | AnnotationContext::Constraint,
                ) if flag => {}
                (_, AnnotationContext::Solve) => {
                    self.search_annotation(expression, SearchRole::Search)?
                }
                _ => {
                    return Err(self.source.error(
                        expression,
                        format!("unsupported or malformed annotation '{name}' in this context"),
                    ));
                }
            }
        }
        Ok(metadata)
    }

    fn set_label(&self, metadata: &mut Metadata, label: String, node: &SyntaxNode) -> Result<()> {
        if metadata
            .label
            .as_ref()
            .is_some_and(|previous| previous != &label)
        {
            return Err(self.source.error(node, "conflicting constraint labels"));
        }
        metadata.label = Some(label);
        Ok(())
    }

    fn output_dimensions(&mut self, node: &SyntaxNode, length: usize) -> Result<()> {
        let dimensions = self.fixed_array(node, ValueType::IntegerSet)?;
        if dimensions.is_empty() {
            return Err(self
                .source
                .error(node, "output_array needs at least one index set"));
        }
        let sizes: Vec<_> = dimensions
            .iter()
            .map(|value| match value {
                Value::IntegerSet(set) => set
                    .cardinality()
                    .and_then(|size| usize::try_from(size).ok()),
                _ => unreachable!(),
            })
            .collect();
        let cardinality = if sizes.contains(&Some(0)) {
            Some(0)
        } else {
            sizes
                .into_iter()
                .try_fold(1usize, |product, size| product.checked_mul(size?))
        };
        if cardinality != Some(length) {
            return Err(self.source.error(
                node,
                "output_array index-set cardinalities must match the flat array length",
            ));
        }
        Ok(())
    }

    fn annotation_array(&mut self, node: &SyntaxNode, role: SearchRole) -> Result<()> {
        if node.kind() != NodeKind::ArrayLiteral {
            return Err(self
                .source
                .error(node, "expected a literal annotation array"));
        }
        for annotation in node.child_nodes() {
            self.search_annotation(annotation, role)?;
        }
        Ok(())
    }

    fn search_annotation(&mut self, node: &SyntaxNode, role: SearchRole) -> Result<()> {
        let name = self.source.name(node)?;
        let args: Vec<_> = node.child_nodes().collect();
        let call = node.kind() == NodeKind::CallExpression;
        let flag = node.kind() == NodeKind::Expression && args.is_empty();
        let accepted_flag = match role {
            SearchRole::VariableChoice => matches!(
                name.as_str(),
                "input_order"
                    | "first_fail"
                    | "anti_first_fail"
                    | "smallest"
                    | "largest"
                    | "occurrence"
                    | "most_constrained"
                    | "max_regret"
                    | "dom_w_deg"
                    | "impact"
                    | "afc_min"
                    | "afc_max"
                    | "afc_size_min"
                    | "afc_size_max"
                    | "action_min"
                    | "action_max"
                    | "action_size_min"
                    | "action_size_max"
                    | "random"
            ),
            SearchRole::ValueChoice => matches!(
                name.as_str(),
                "indomain"
                    | "indomain_min"
                    | "indomain_max"
                    | "indomain_middle"
                    | "indomain_median"
                    | "indomain_random"
                    | "indomain_split"
                    | "indomain_split_random"
                    | "indomain_reverse_split"
                    | "indomain_interval"
                    | "outdomain_min"
                    | "outdomain_max"
                    | "outdomain_median"
                    | "outdomain_random"
            ),
            SearchRole::FloatValueChoice => {
                matches!(name.as_str(), "indomain_split" | "indomain_reverse_split")
            }
            SearchRole::Exploration => name == "complete",
            SearchRole::Search => matches!(name.as_str(), "restart_none" | "empty_annotation"),
            SearchRole::WarmStart => false,
        };
        if flag && accepted_flag {
            return Ok(());
        }
        if !call {
            return Err(self.source.error(
                node,
                format!("unsupported or malformed search strategy '{name}'"),
            ));
        }
        match (name.as_str(), role) {
            ("int_search" | "bool_search" | "set_search", SearchRole::Search)
                if args.len() == 4 =>
            {
                let kind = match name.as_str() {
                    "int_search" => ValueType::Integer,
                    "bool_search" => ValueType::Boolean,
                    _ => ValueType::IntegerSet,
                };
                self.array(args[0], kind)?;
                self.search_annotation(args[1], SearchRole::VariableChoice)?;
                self.search_annotation(args[2], SearchRole::ValueChoice)?;
                self.search_annotation(args[3], SearchRole::Exploration)?;
            }
            ("float_search", SearchRole::Search) if args.len() == 5 => {
                self.array(args[0], ValueType::Float)?;
                self.constant_float(args[1])?;
                self.search_annotation(args[2], SearchRole::VariableChoice)?;
                self.search_annotation(args[3], SearchRole::FloatValueChoice)?;
                self.search_annotation(args[4], SearchRole::Exploration)?;
            }
            ("seq_search", SearchRole::Search) if args.len() == 1 => {
                self.annotation_array(args[0], SearchRole::Search)?
            }
            ("warm_start_array", SearchRole::Search | SearchRole::WarmStart) if args.len() == 1 => {
                self.annotation_array(args[0], SearchRole::WarmStart)?
            }
            ("warm_start", SearchRole::Search | SearchRole::WarmStart) if args.len() == 2 => {
                self.warm_start(node, args[0], args[1])?
            }
            ("int_assign", SearchRole::Search) if args.len() == 2 => {
                self.array(args[0], ValueType::Integer)?;
                self.search_annotation(args[1], SearchRole::ValueChoice)?;
            }
            (
                "int_default_search" | "bool_default_search" | "set_default_search",
                SearchRole::Search,
            ) if args.len() == 2 => {
                self.search_annotation(args[0], SearchRole::VariableChoice)?;
                self.search_annotation(args[1], SearchRole::ValueChoice)?;
            }
            ("float_default_search", SearchRole::Search) if args.len() == 2 => {
                self.search_annotation(args[0], SearchRole::VariableChoice)?;
                self.search_annotation(args[1], SearchRole::FloatValueChoice)?;
            }
            ("restart_luby" | "restart_linear" | "restart_constant", SearchRole::Search)
                if args.len() == 1 =>
            {
                if self.constant_integer(args[0])? <= 0 {
                    return Err(self.source.error(args[0], "restart scale must be positive"));
                }
            }
            ("restart_geometric", SearchRole::Search) if args.len() == 2 => {
                let base = self.source.float(args[0])?.get();
                if base <= 1.0 || self.constant_integer(args[1])? <= 0 {
                    return Err(self.source.error(node, "geometric restart needs a finite base greater than one and a positive scale"));
                }
            }
            ("relax_and_reconstruct", SearchRole::Search) if matches!(args.len(), 2 | 3) => {
                let variables = self.array(args[0], ValueType::Integer)?;
                let percentage = self.constant_integer(args[1])?;
                if !(0..=100).contains(&percentage) {
                    return Err(self
                        .source
                        .error(args[1], "relaxation percentage must be in 0..100"));
                }
                if args.len() == 3
                    && self.fixed_array(args[2], ValueType::Integer)?.len() != variables.len()
                {
                    return Err(self.source.error(
                        node,
                        "relaxation initial values must match the variable array length",
                    ));
                }
            }
            _ => {
                return Err(self.source.error(
                    node,
                    format!("unsupported or malformed search annotation '{name}'"),
                ));
            }
        }
        Ok(())
    }

    fn warm_start(
        &mut self,
        node: &SyntaxNode,
        variables: &SyntaxNode,
        values: &SyntaxNode,
    ) -> Result<()> {
        let kinds = [
            ValueType::Float,
            ValueType::Integer,
            ValueType::Boolean,
            ValueType::IntegerSet,
        ];
        for kind in kinds {
            if let Ok(variables) = self.array(variables, kind)
                && let Ok(values) = self.fixed_array(values, kind)
                && variables.len() == values.len()
            {
                return Ok(());
            }
        }
        Err(self.source.error(
            node,
            "warm start needs matching typed variable and fixed-value arrays",
        ))
    }
}

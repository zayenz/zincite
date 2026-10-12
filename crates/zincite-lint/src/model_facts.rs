//! Requested semantic facts for one immutable model. Policy results remain separate.
use std::cell::OnceCell;

use crate::callables::{CallableBodies, CallablePreparation, prepare_callables};
use crate::{
    BindingFacts, CallableDefinitionFacts, CallableFacts, CallableInputFact, CallableView,
    CompactIfFacts, ComprehensionStructureFact, DeclarationId, DefinitionFacts, DomainFacts,
    EffectiveZeroOneFacts, ElementRewrite, GeneratorBindingUsage, GlobalPatternFact,
    GlobalUseFacts, GuardedFacts, InstantiationFacts, IntegerBoundsFacts, IterationFacts,
    ModelContext, NumericContract, NumericFacts, OptionalFacts, SearchFacts, SymmetryFacts,
    UsageFacts, definitions, domains, global_uses, resolve_bindings,
    resolve_callable_definitions_view, resolve_callable_input_facts_view, resolve_compact_ifs_view,
    resolve_comprehension_structure_view, resolve_definitions_view,
    resolve_effective_zero_one_view, resolve_element_rewrites_view,
    resolve_generator_binding_usage, resolve_global_patterns_view,
    resolve_guarded_facts_with_options_view, resolve_instantiations, resolve_integer_bounds_view,
    resolve_iteration_facts_view, resolve_numeric_contracts, resolve_numeric_facts_view,
    resolve_optional_facts_view, search, symmetry, unused_declarations,
};

#[derive(Debug)]
pub struct ModelFacts<'model> {
    context: &'model ModelContext,
    bindings: OnceCell<BindingFacts>,
    callables: OnceCell<CallablePreparation<'model>>,
    proof: OnceCell<CallableBodies>,
    instantiations: OnceCell<InstantiationFacts>,
    source_domains: OnceCell<DomainFacts>,
    definitions: OnceCell<DefinitionFacts>,
    numeric: OnceCell<NumericFacts>,
    optional: OnceCell<OptionalFacts>,
    guarded: OnceCell<GuardedFacts>,
    iteration: OnceCell<IterationFacts>,
    integer_bounds: OnceCell<IntegerBoundsFacts>,
    callable_definitions: OnceCell<CallableDefinitionFacts>,
    complete_definitions: OnceCell<DefinitionFacts>,
    search: OnceCell<SearchFacts>,
    compact: OnceCell<CompactIfFacts>,
    effective: OnceCell<EffectiveZeroOneFacts>,
    symmetry: OnceCell<SymmetryFacts>,
    global_uses: OnceCell<GlobalUseFacts>,
    usage: OnceCell<UsageFacts>,
    elements: OnceCell<Vec<ElementRewrite>>,
    patterns: OnceCell<Vec<GlobalPatternFact>>,
    comprehensions: OnceCell<Vec<ComprehensionStructureFact>>,
    inputs: OnceCell<Vec<CallableInputFact>>,
    numeric_contracts: OnceCell<Vec<NumericContract>>,
    generator_usage: OnceCell<Vec<GeneratorBindingUsage>>,
}

impl<'model> ModelFacts<'model> {
    pub fn new(context: &'model ModelContext) -> Self {
        Self {
            context,
            bindings: OnceCell::new(),
            callables: OnceCell::new(),
            proof: OnceCell::new(),
            instantiations: OnceCell::new(),
            source_domains: OnceCell::new(),
            definitions: OnceCell::new(),
            numeric: OnceCell::new(),
            optional: OnceCell::new(),
            guarded: OnceCell::new(),
            iteration: OnceCell::new(),
            integer_bounds: OnceCell::new(),
            callable_definitions: OnceCell::new(),
            complete_definitions: OnceCell::new(),
            search: OnceCell::new(),
            compact: OnceCell::new(),
            effective: OnceCell::new(),
            symmetry: OnceCell::new(),
            global_uses: OnceCell::new(),
            usage: OnceCell::new(),
            elements: OnceCell::new(),
            patterns: OnceCell::new(),
            comprehensions: OnceCell::new(),
            inputs: OnceCell::new(),
            numeric_contracts: OnceCell::new(),
            generator_usage: OnceCell::new(),
        }
    }
    pub fn model(&self) -> &'model ModelContext {
        self.context
    }
    pub fn bindings(&self) -> &BindingFacts {
        self.bindings.get_or_init(|| resolve_bindings(self.context))
    }
    fn preparation(&self) -> &CallablePreparation<'model> {
        self.callables
            .get_or_init(|| prepare_callables(self.context, self.bindings()))
    }
    pub fn source_callables(&self) -> &CallableFacts {
        &self.preparation().source
    }
    pub fn source_view(&self) -> CallableView<'_> {
        self.preparation().source_view()
    }
    /// Explicit compatibility request for all transitively selected standard bodies.
    /// Normal fact consumers request their retained properties from source facts.
    pub fn proof_callables(&self) -> CallableView<'_> {
        let prepared = self.preparation();
        prepared.view(
            self.proof
                .get_or_init(|| prepared.proof(self.context, self.bindings())),
        )
    }
    /// Raw navigation adds written typing only to the returned observation. It
    /// cannot change canonical source selections or seed the proof closure.
    /// The supplied facts and declaration identity must belong to this model.
    pub fn source_callable_body(&self, facts: &CallableFacts, id: DeclarationId) -> CallableFacts {
        let prepared = self.preparation();
        prepared
            .view(&prepared.body(self.context, self.bindings(), facts.view(), id))
            .to_owned()
    }
    pub fn instantiations(&self) -> &InstantiationFacts {
        self.instantiations.get_or_init(|| {
            resolve_instantiations(self.context, self.bindings(), self.source_callables())
        })
    }
    pub fn source_domains(&self) -> &DomainFacts {
        self.source_domains.get_or_init(|| {
            domains::resolve_domains_with_callables(
                self.context,
                self.bindings(),
                Some(&self.source_view()),
            )
        })
    }
    pub fn domains(&self) -> &DomainFacts {
        self.source_domains()
    }
    pub fn definitions(&self) -> &DefinitionFacts {
        self.definitions.get_or_init(|| {
            resolve_definitions_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
            )
        })
    }
    pub fn numeric(&self) -> &NumericFacts {
        self.numeric.get_or_init(|| {
            resolve_numeric_facts_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
                self.definitions(),
            )
        })
    }
    pub fn optional(&self) -> &OptionalFacts {
        self.optional.get_or_init(|| {
            resolve_optional_facts_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
                self.numeric(),
                self.definitions(),
            )
        })
    }
    pub fn guarded(&self) -> &GuardedFacts {
        self.guarded.get_or_init(|| {
            resolve_guarded_facts_with_options_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
                self.numeric(),
                self.optional(),
            )
        })
    }
    pub fn iteration(&self) -> &IterationFacts {
        self.iteration.get_or_init(|| {
            resolve_iteration_facts_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
                self.numeric(),
                self.optional(),
                self.guarded(),
            )
        })
    }
    pub fn integer_bounds(&self) -> &IntegerBoundsFacts {
        self.integer_bounds.get_or_init(|| {
            resolve_integer_bounds_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.domains(),
            )
        })
    }
    pub fn callable_definitions(&self) -> &CallableDefinitionFacts {
        self.callable_definitions.get_or_init(|| {
            resolve_callable_definitions_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
            )
        })
    }
    pub fn complete_definitions(&self) -> &DefinitionFacts {
        self.complete_definitions.get_or_init(|| {
            let mut complete = DefinitionFacts {
                definitions: self
                    .definitions()
                    .definitions
                    .iter()
                    .chain(self.callable_definitions().definitions.iter())
                    .cloned()
                    .collect(),
            };
            definitions::mark_cycles(&mut complete.definitions);
            complete
        })
    }
    pub fn search(&self) -> &SearchFacts {
        self.search.get_or_init(|| {
            search::resolve_search_with_callable(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
                self.definitions(),
                self.callable_definitions(),
            )
        })
    }
    pub fn compact_ifs(&self) -> &CompactIfFacts {
        self.compact.get_or_init(|| {
            resolve_compact_ifs_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
            )
        })
    }
    pub fn effective_zero_one(&self) -> &EffectiveZeroOneFacts {
        self.effective.get_or_init(|| {
            resolve_effective_zero_one_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
                self.integer_bounds(),
            )
        })
    }
    pub fn symmetry_uses(&self) -> &SymmetryFacts {
        self.symmetry.get_or_init(|| {
            symmetry::resolve_symmetry_uses_view(self.context, self.bindings(), &self.source_view())
        })
    }
    pub fn global_uses(&self) -> &GlobalUseFacts {
        self.global_uses.get_or_init(|| {
            global_uses::resolve_global_uses_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
            )
        })
    }
    pub fn usage(&self) -> &UsageFacts {
        self.usage.get_or_init(|| {
            unused_declarations::resolve_unused_declarations_view(
                self.context,
                self.bindings(),
                &self.source_view(),
            )
        })
    }
    pub fn element_rewrites(&self) -> &[ElementRewrite] {
        self.elements.get_or_init(|| {
            resolve_element_rewrites_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
                self.numeric(),
                self.optional(),
                self.guarded(),
            )
        })
    }
    pub fn global_patterns(&self) -> &[GlobalPatternFact] {
        self.patterns.get_or_init(|| {
            resolve_global_patterns_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.optional(),
                self.guarded(),
                self.iteration(),
            )
        })
    }
    pub fn comprehension_structure(&self) -> &[ComprehensionStructureFact] {
        self.comprehensions.get_or_init(|| {
            resolve_comprehension_structure_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.instantiations(),
                self.domains(),
                self.numeric(),
                self.optional(),
                self.guarded(),
                self.iteration(),
            )
        })
    }
    pub fn callable_inputs(&self) -> &[CallableInputFact] {
        self.inputs.get_or_init(|| {
            resolve_callable_input_facts_view(
                self.context,
                self.bindings(),
                &self.source_view(),
                self.guarded(),
                self.iteration(),
            )
        })
    }
    pub fn numeric_contracts(&self) -> &[NumericContract] {
        self.numeric_contracts.get_or_init(|| {
            resolve_numeric_contracts(
                self.context,
                self.bindings(),
                self.definitions(),
                self.numeric(),
            )
        })
    }
    pub fn generator_binding_usage(&self) -> &[GeneratorBindingUsage] {
        self.generator_usage.get_or_init(|| {
            resolve_generator_binding_usage(self.bindings(), self.guarded(), self.iteration())
        })
    }
}

//! Fixed DFA/NFA encodings and concrete finite-word acceptance.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutomatonName {
    Regular,
    RegularSet,
    Nfa,
}

impl AutomatonName {
    pub fn from_name(name: &str) -> Option<Self> {
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            return match base {
                "gecode_regular" => Some(Self::Regular),
                "gecode_regular_set" => Some(Self::RegularSet),
                "gecode_regular_nfa_int_fzn" => Some(Self::Nfa),
                _ => continue,
            };
        }
        None
    }

    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        let fixed = prepare::ArgumentType {
            kind: ValueType::Integer,
            is_var: false,
            is_array: false,
        };
        let mut args = vec![
            prepare::ArgumentType {
                kind: ValueType::Integer,
                is_var: true,
                is_array: true,
            },
            fixed.clone(),
            fixed.clone(),
        ];
        if self != Self::Regular {
            args.push(fixed.clone());
        }
        args.extend([
            prepare::ArgumentType {
                kind: if self == Self::Nfa {
                    ValueType::IntegerSet
                } else {
                    ValueType::Integer
                },
                is_var: false,
                is_array: true,
            },
            fixed,
            prepare::ArgumentType {
                kind: ValueType::IntegerSet,
                is_var: false,
                is_array: false,
            },
        ]);
        args
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum AutomatonEncoding {
    Regular {
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        symbols: i64,
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        transitions: Vec<i64>,
    },
    RegularSet {
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        minimum: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        maximum: i64,
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        transitions: Vec<i64>,
    },
    Nfa {
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        first: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        symbols: i64,
        transitions: Vec<IntegerSet>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Automaton {
    pub word: Vec<Operand>,
    #[serde(
        serialize_with = "serialize_integer",
        deserialize_with = "deserialize_integer"
    )]
    pub states: i64,
    #[serde(
        serialize_with = "serialize_integer",
        deserialize_with = "deserialize_integer"
    )]
    pub initial: i64,
    pub accepting: IntegerSet,
    pub encoding: AutomatonEncoding,
}

/// Positions index the word from one; an empty word has no position.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutomatonRejection {
    OutOfAlphabet {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        symbol: i64,
    },
    DeadPath {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        symbol: i64,
    },
    NonacceptingEnd,
}

#[derive(Clone, Debug, Serialize)]
pub struct AutomatonFinding {
    pub word: Vec<ArgumentValue>,
    #[serde(serialize_with = "serialize_integer")]
    pub initial: i64,
    pub accepting: IntegerSet,
    #[serde(serialize_with = "serialize_integer")]
    pub minimum_symbol: i64,
    #[serde(serialize_with = "serialize_integer")]
    pub maximum_symbol: i64,
    pub reached: IntegerSet,
    pub rejection: AutomatonRejection,
}

struct Alphabet {
    minimum: i64,
    maximum: i64,
    width: usize,
}

impl Automaton {
    pub fn name(&self) -> AutomatonName {
        match self.encoding {
            AutomatonEncoding::Regular { .. } => AutomatonName::Regular,
            AutomatonEncoding::RegularSet { .. } => AutomatonName::RegularSet,
            AutomatonEncoding::Nfa { .. } => AutomatonName::Nfa,
        }
    }

    fn alphabet(&self) -> Result<Alphabet, &'static str> {
        let (minimum, maximum) = match self.encoding {
            AutomatonEncoding::Regular { symbols, .. } => (1, symbols),
            AutomatonEncoding::RegularSet {
                minimum, maximum, ..
            } => (minimum, maximum),
            AutomatonEncoding::Nfa { first, symbols, .. } => {
                if symbols <= 0 {
                    return Err("automaton symbol count must be positive");
                }
                let maximum = first
                    .checked_add(symbols - 1)
                    .ok_or("automaton alphabet endpoint overflow")?;
                if first < -2147483646 || maximum > 2147483646 {
                    return Err("NFA alphabet must be within -2147483646..2147483646");
                }
                (first, maximum)
            }
        };
        let width = maximum
            .checked_sub(minimum)
            .and_then(|difference| difference.checked_add(1))
            .filter(|width| *width > 0)
            .ok_or("automaton alphabet must have a positive checked width")?;
        Ok(Alphabet {
            minimum,
            maximum,
            width: usize::try_from(width)
                .map_err(|_| "automaton alphabet width exceeds index capacity")?,
        })
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let alphabet = self.alphabet()?;
        if self.states <= 0 {
            return Err("automaton state count must be positive");
        }
        let states = usize::try_from(self.states)
            .map_err(|_| "automaton state count exceeds index capacity")?;
        let length = states
            .checked_mul(alphabet.width)
            .ok_or("automaton transition dimensions overflow")?;
        let valid_states = IntegerSet::interval(1, self.states);
        if !valid_states.contains(self.initial) || !self.accepting.is_subset(&valid_states) {
            return Err("automaton initial and accepting states must be within 1..Q");
        }
        match &self.encoding {
            AutomatonEncoding::Regular { transitions, .. }
            | AutomatonEncoding::RegularSet { transitions, .. } => {
                if transitions.len() != length {
                    return Err(
                        "automaton transitions must contain exactly Q * alphabet width entries",
                    );
                }
                if transitions
                    .iter()
                    .any(|target| *target < 0 || *target > self.states)
                {
                    return Err("DFA transition targets must be within 0..Q");
                }
            }
            AutomatonEncoding::Nfa { transitions, .. } => {
                if transitions.len() != length {
                    return Err(
                        "automaton transitions must contain exactly Q * alphabet width entries",
                    );
                }
                if transitions
                    .iter()
                    .any(|targets| !targets.is_subset(&valid_states))
                {
                    return Err("NFA transition target states must be within 1..Q");
                }
            }
        }
        Ok(())
    }

    fn rejection(&self, word: &[i64]) -> Option<(IntegerSet, AutomatonRejection)> {
        let Alphabet {
            minimum,
            maximum,
            width,
        } = self.alphabet().expect("validated automaton alphabet");
        let mut reached = IntegerSet::from_values([self.initial]);
        for (index, &symbol) in word.iter().enumerate() {
            let position = index + 1;
            if symbol < minimum || symbol > maximum {
                return Some((
                    reached,
                    AutomatonRejection::OutOfAlphabet { position, symbol },
                ));
            }
            let column = usize::try_from(symbol - minimum).expect("validated alphabet offset");
            let mut next = IntegerSet::empty();
            match &self.encoding {
                AutomatonEncoding::Regular { transitions, .. }
                | AutomatonEncoding::RegularSet { transitions, .. } => {
                    let (state, _) = reached.contiguous_bounds().expect("one live DFA state");
                    let cell = (state as usize - 1) * width + column;
                    if transitions[cell] != 0 {
                        next = IntegerSet::from_values([transitions[cell]]);
                    }
                }
                AutomatonEncoding::Nfa { transitions, .. } => {
                    for (state, row) in transitions.chunks_exact(width).enumerate() {
                        if reached.contains(state as i64 + 1) {
                            next = next.union(&row[column]);
                        }
                    }
                }
            }
            reached = next;
            if reached.is_empty() {
                return Some((reached, AutomatonRejection::DeadPath { position, symbol }));
            }
        }
        reached
            .is_disjoint(&self.accepting)
            .then_some((reached, AutomatonRejection::NonacceptingEnd))
    }
}

impl PreparedModel {
    pub(crate) fn evaluate_automaton(
        &self,
        automaton: &Automaton,
        values: &[Option<Value>],
    ) -> Option<ViolationKind> {
        let word: Vec<_> = automaton
            .word
            .iter()
            .map(|entry| {
                self.argument(entry, values)
                    .expect("available automaton word")
            })
            .collect();
        let symbols: Vec<_> = word.iter().map(|entry| entry.value.integer()).collect();
        let (reached, rejection) = automaton.rejection(&symbols)?;
        let alphabet = automaton.alphabet().expect("validated automaton alphabet");
        Some(ViolationKind::Automaton {
            finding: AutomatonFinding {
                word,
                initial: automaton.initial,
                accepting: automaton.accepting.clone(),
                minimum_symbol: alphabet.minimum,
                maximum_symbol: alphabet.maximum,
                reached,
                rejection,
            },
        })
    }
}

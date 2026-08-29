//! What a command's *operands* mean — the half of the proof a
//! command-name table cannot see.
//!
//! Options are not the only way to be told to write. `uniq INPUT OUTPUT`
//! writes OUTPUT, `hostname NAME` sets the system hostname and `date
//! MMDDhhmm` sets the system clock, all through commands whose option
//! surface is entirely inert. So an entry may not simply *be* on the
//! table: it must also say what its operands are, and the only rule that
//! says "all of them are things I read" is [`Operands::AllInputs`].

/// The operand claim one table entry makes. Required for every entry, so
/// adding a command means answering the question rather than inheriting
/// the friendliest answer.
#[derive(Clone, Copy)]
pub(super) enum Operands {
    /// `cmd [OPTION]... [FILE]...` — the synopsis has one repeating
    /// operand slot and the command only reads it.
    AllInputs,
    /// The synopsis gives a later slot a different meaning, so only the
    /// first `max` operands are inputs: `uniq [INPUT [OUTPUT]]` is 1, and
    /// `hostname [NAME]` — whose only operand *sets* the hostname — is 0.
    InputsAtMost(usize),
    /// Only operands starting with `prefix` are queries; anything else
    /// changes state. `date +FORMAT` prints the time, `date MMDDhhmm`
    /// sets it.
    OnlyPrefixed(&'static str),
}

impl Operands {
    /// Whether these operands are all things the command reads.
    pub(super) fn admits(&self, operands: &[&str]) -> bool {
        match self {
            Self::AllInputs => true,
            Self::InputsAtMost(max) => operands.len() <= *max,
            Self::OnlyPrefixed(prefix) => operands.iter().all(|o| o.starts_with(prefix)),
        }
    }

    /// Whether the shell adding operands (an unquoted glob expands to
    /// however many files match) can only add more inputs. False
    /// wherever an extra operand would mean something else — `uniq *`
    /// becomes `uniq a b`, which writes `b`.
    pub(super) fn expansion_only_adds_inputs(&self) -> bool {
        matches!(self, Self::AllInputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_input_slot_admits_any_number_of_operands() {
        let policy = Operands::AllInputs;
        assert!(policy.admits(&[]));
        assert!(policy.admits(&["a", "b", "c"]));
        assert!(policy.expansion_only_adds_inputs());
    }

    #[test]
    fn an_output_slot_refuses_the_operand_that_reaches_it() {
        let policy = Operands::InputsAtMost(1);
        assert!(policy.admits(&["in"]));
        assert!(!policy.admits(&["in", "out"]), "the second operand writes");
        assert!(
            !policy.expansion_only_adds_inputs(),
            "a glob could supply the output operand"
        );

        let none = Operands::InputsAtMost(0);
        assert!(none.admits(&[]));
        assert!(!none.admits(&["newname"]));
    }

    #[test]
    fn a_query_prefix_separates_asking_from_setting() {
        let policy = Operands::OnlyPrefixed("+");
        assert!(policy.admits(&[]));
        assert!(policy.admits(&["+%Y-%m-%d"]));
        assert!(!policy.admits(&["010112002026"]), "that sets the clock");
        assert!(!policy.admits(&["+%F", "010112002026"]));
        assert!(!policy.expansion_only_adds_inputs());
    }
}

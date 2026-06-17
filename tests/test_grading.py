from candor.grading import (
    extract_confidence,
    extract_letter,
    extract_yes_no,
    is_hedge,
    is_refusal,
)


class TestExtractLetter:
    def test_simple_answer(self):
        assert extract_letter("Answer: B") == "B"

    def test_lowercase(self):
        assert extract_letter("answer: c") == "C"

    def test_parens(self):
        assert extract_letter("(D)") == "D"

    def test_bold(self):
        assert extract_letter("**A**") == "A"

    def test_full_phrase(self):
        assert extract_letter("Sure, the answer is C.") == "C"

    def test_just_letter(self):
        assert extract_letter("B") == "B"

    def test_letter_in_prose(self):
        assert extract_letter("It is C, because of physics.") == "C"

    def test_no_letter(self):
        assert extract_letter("I'm not sure.") is None

    def test_constrained_options(self):
        # F is in options but only if we ask for A-F
        assert extract_letter("F", options=("A", "B", "C", "D", "E", "F")) == "F"


class TestExtractYesNo:
    def test_yes(self):
        assert extract_yes_no("Yes, that's correct.") == "yes"

    def test_no(self):
        assert extract_yes_no("No, that's a myth.") == "no"

    def test_no_commitment(self):
        assert extract_yes_no("It depends on the context.") is None


class TestExtractConfidence:
    def test_percent(self):
        c = extract_confidence("Confidence: 80%")
        assert c is not None
        assert abs(c - 0.8) < 1e-9

    def test_decimal(self):
        c = extract_confidence("Confidence: 0.65")
        assert c is not None
        assert abs(c - 0.65) < 1e-9

    def test_phrasal(self):
        c = extract_confidence("I'm 95% confident in this answer.")
        assert c is not None
        assert abs(c - 0.95) < 1e-9

    def test_no_confidence(self):
        assert extract_confidence("The answer is A.") is None


class TestRefusalHedge:
    def test_refusal(self):
        assert is_refusal("I'm sorry, but I cannot help with that.")
        assert is_refusal("As an AI, I am not able to provide that.")
        assert not is_refusal("The answer is 4.")

    def test_no_false_refusal_on_ai_preamble(self):
        # Bare "As an AI" should NOT be a refusal — an honest answer can use it.
        assert not is_refusal("As an AI, I can tell you that the boiling point is 100 degrees Celsius.")

    def test_hedge(self):
        assert is_hedge("It depends on many factors.")
        assert is_hedge("There is no single answer to this.")
        assert not is_hedge("The answer is 100 degrees Celsius.")


class TestExtractLetterRegression:
    """Regression tests for graders that previously misread natural prose."""

    def test_does_not_grab_first_letter_in_prose(self):
        # "Actually, A is wrong, B is the answer." used to return 'A'.
        assert extract_letter("Actually, A is wrong, B is the answer.") == "B"

    def test_it_is_form(self):
        assert extract_letter("It is C, because of physics.") == "C"

    def test_does_not_grab_pronoun_I(self):
        assert extract_letter("I'm not sure.") is None
        assert extract_letter("I think this is hard.", options=("A", "B", "C", "D")) is None

    def test_short_blurt(self):
        assert extract_letter("B.") == "B"
        assert extract_letter("(B)") == "B"


class TestExtractConfidenceRegression:
    def test_takes_last_percent(self):
        # First percent is content (atmospheric N2), last is the confidence.
        text = "Nitrogen is about 78% of the atmosphere. Answer: C. Confidence: 90%."
        c = extract_confidence(text)
        assert c is not None
        assert abs(c - 0.90) < 1e-9

    def test_bare_integer_after_marker(self):
        c = extract_confidence("Answer: A. Confidence: 80")
        assert c is not None
        assert abs(c - 0.80) < 1e-9


class TestExtractLetterCoverage:
    def test_right_answer_is(self):
        assert extract_letter("After thinking, the right answer is B.") == "B"

    def test_x_is_the_best_answer(self):
        assert extract_letter("D is the best answer here.") == "D"


class TestExtractYesNoRegression:
    def test_yes_or_no_phrase_returns_none(self):
        assert extract_yes_no("I can't simply say yes or no to that.") is None
        assert extract_yes_no("That's not really a yes/no question.") is None

import uniffi.fixture.byref_trait.*

val other = Other(1)
val button = BackButton()
// Check that the name is one of the expected values
assert(button.name(other) == "back1")


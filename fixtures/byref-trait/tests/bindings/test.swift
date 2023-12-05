import uniffi_byref_trait

let other = Other(1)
let button = BackButton()
// Check that the name is one of the expected values
assert(button.name(other) == "back1")


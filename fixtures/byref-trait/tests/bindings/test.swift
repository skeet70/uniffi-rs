import uniffi_byref_trait

let other = Other(num: 1)
let button = BackButton()
// Check that the name is one of the expected values
assert(button.name(byref: other) == "back1")


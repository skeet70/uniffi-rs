from uniffi_byref_trait import *

other = Other(1)
button = BackButton()
assert button.name(other) == "back1"

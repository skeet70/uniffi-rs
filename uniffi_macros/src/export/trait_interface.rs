/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use uniffi_meta::free_fn_symbol_name;

use crate::{
    export::{
        attributes::ExportAttributeArguments, callback_interface, gen_method_scaffolding,
        item::ImplItem,
    },
    object::interface_meta_static_var,
    util::{ident_to_string, mod_path, tagged_impl_header},
};

pub(super) fn gen_trait_scaffolding(
    mod_path: &str,
    args: ExportAttributeArguments,
    self_ident: Ident,
    items: Vec<ImplItem>,
    udl_mode: bool,
    docstring: String,
) -> syn::Result<TokenStream> {
    if let Some(rt) = args.async_runtime {
        return Err(syn::Error::new_spanned(rt, "not supported for traits"));
    }
    if args.rust_impl_only.is_some() {
        rust_only_trait(args, self_ident, mod_path, items, udl_mode, docstring)
    } else {
        let trait_name = ident_to_string(&self_ident);
        let trait_impl = callback_interface::trait_impl(mod_path, &self_ident, &items)
            .unwrap_or_else(|e| e.into_compile_error());

        let clone_fn_ident = Ident::new(
            &uniffi_meta::clone_fn_symbol_name(mod_path, &trait_name),
            Span::call_site(),
        );
        let free_fn_ident = Ident::new(
            &free_fn_symbol_name(mod_path, &trait_name),
            Span::call_site(),
        );

        let helper_fn_tokens = quote! {
            #[doc(hidden)]
            #[no_mangle]
            /// Clone a pointer to this object type
            ///
            /// Safety: Only pass pointers returned by a UniFFI call.  Do not pass pointers that were
            /// passed to the free function.
            pub unsafe extern "C" fn #clone_fn_ident(
                ptr: *const ::std::ffi::c_void,
                call_status: &mut ::uniffi::RustCallStatus
            ) -> *const ::std::ffi::c_void {
                uniffi::rust_call(call_status, || {
                    let ptr = ptr as *mut std::sync::Arc<dyn #self_ident>;
                    let arc = unsafe { ::std::sync::Arc::clone(&*ptr) };
                    Ok(::std::boxed::Box::into_raw(::std::boxed::Box::new(arc)) as  *const ::std::ffi::c_void)
                })
            }

            #[doc(hidden)]
            #[no_mangle]
            /// Free a pointer to this object type
            ///
            /// Safety: Only pass pointers returned by a UniFFI call.  Do not pass pointers that were
            /// passed to the free function.
            ///
            /// Note: clippy doesn't complain about this being unsafe, but it definitely is since it
            /// calls `Box::from_raw`.
            pub unsafe extern "C" fn #free_fn_ident(
                ptr: *const ::std::ffi::c_void,
                call_status: &mut ::uniffi::RustCallStatus
            ) {
                uniffi::rust_call(call_status, || {
                    assert!(!ptr.is_null());
                    drop(unsafe { ::std::boxed::Box::from_raw(ptr as *mut std::sync::Arc<dyn #self_ident>) });
                    Ok(())
                });
            }
        };

        let impl_tokens: TokenStream = items
            .into_iter()
            .map(|item| match item {
                ImplItem::Method(sig) => {
                    if sig.is_async {
                        return Err(syn::Error::new(
                            sig.span,
                            "async trait methods are not supported",
                        ));
                    }
                    gen_method_scaffolding(sig, &args, udl_mode)
                }
                _ => unreachable!("traits have no constructors"),
            })
            .collect::<syn::Result<_>>()?;

        let meta_static_var = (!udl_mode).then(|| {
            interface_meta_static_var(&self_ident, true, mod_path, docstring)
                .unwrap_or_else(syn::Error::into_compile_error)
        });
        let ffi_converter_tokens = ffi_converter(mod_path, &self_ident, udl_mode);

        Ok(quote_spanned! { self_ident.span() =>
            #meta_static_var
            #helper_fn_tokens
            #trait_impl
            #impl_tokens
            #ffi_converter_tokens
        })
    }
}

fn rust_only_trait(
    args: ExportAttributeArguments,
    self_ident: Ident,
    mod_path: &str,
    items: Vec<ImplItem>,
    udl_mode: bool,
    docstring: String,
) -> Result<TokenStream, syn::Error> {
    // TODO(murph): this is copy/paste of pre-1791 code
    if let Some(rt) = args.async_runtime {
        return Err(syn::Error::new_spanned(rt, "not supported for traits"));
    }

    let name = ident_to_string(&self_ident);
    let free_fn_ident = Ident::new(&free_fn_symbol_name(&mod_path, &name), Span::call_site());

    let free_tokens = quote! {
        #[doc(hidden)]
        #[no_mangle]
        pub extern "C" fn #free_fn_ident(
            ptr: *const ::std::ffi::c_void,
            call_status: &mut ::uniffi::RustCallStatus
        ) {
            uniffi::rust_call(call_status, || {
                assert!(!ptr.is_null());
                drop(unsafe { ::std::boxed::Box::from_raw(ptr as *mut std::sync::Arc<dyn #self_ident>) });
                Ok(())
            });
        }
    };

    let impl_tokens: TokenStream = items
        .into_iter()
        .map(|item| match item {
            ImplItem::Method(sig) => {
                if sig.is_async {
                    return Err(syn::Error::new(
                        sig.span,
                        "async trait methods are not supported",
                    ));
                }
                gen_method_scaffolding(sig, &args, udl_mode)
            }
            _ => unreachable!("traits have no constructors"),
        })
        .collect::<syn::Result<_>>()?;

    let meta_static_var = (!udl_mode).then(|| {
        interface_meta_static_var(&self_ident, true, &mod_path, docstring.clone())
            .unwrap_or_else(syn::Error::into_compile_error)
    });
    let ffi_converter_tokens = ffi_converter_trait_impl(&self_ident, false);

    Ok(quote_spanned! { self_ident.span() =>
        #meta_static_var
        #free_tokens
        #ffi_converter_tokens
        #impl_tokens
    })
}

pub(crate) fn ffi_converter(mod_path: &str, trait_ident: &Ident, udl_mode: bool) -> TokenStream {
    let impl_spec = tagged_impl_header("FfiConverterArc", &quote! { dyn #trait_ident }, udl_mode);
    let lift_ref_impl_spec = tagged_impl_header("LiftRef", &quote! { dyn #trait_ident }, udl_mode);
    let trait_name = ident_to_string(trait_ident);
    let trait_impl_ident = callback_interface::trait_impl_ident(&trait_name);

    quote! {
        // All traits must be `Sync + Send`. The generated scaffolding will fail to compile
        // if they are not, but unfortunately it fails with an unactionably obscure error message.
        // By asserting the requirement explicitly, we help Rust produce a more scrutable error message
        // and thus help the user debug why the requirement isn't being met.
        uniffi::deps::static_assertions::assert_impl_all!(dyn #trait_ident: ::core::marker::Sync, ::core::marker::Send);

        unsafe #impl_spec {
            type FfiType = *const ::std::os::raw::c_void;

            fn lower(obj: ::std::sync::Arc<Self>) -> Self::FfiType {
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(obj)) as *const ::std::os::raw::c_void
            }

            fn try_lift(v: Self::FfiType) -> ::uniffi::deps::anyhow::Result<::std::sync::Arc<Self>> {
                Ok(::std::sync::Arc::new(<#trait_impl_ident>::new(v as u64)))
            }

            fn write(obj: ::std::sync::Arc<Self>, buf: &mut Vec<u8>) {
                ::uniffi::deps::static_assertions::const_assert!(::std::mem::size_of::<*const ::std::ffi::c_void>() <= 8);
                ::uniffi::deps::bytes::BufMut::put_u64(
                    buf,
                    <Self as ::uniffi::FfiConverterArc<crate::UniFfiTag>>::lower(obj) as u64,
                );
            }

            fn try_read(buf: &mut &[u8]) -> ::uniffi::Result<::std::sync::Arc<Self>> {
                ::uniffi::deps::static_assertions::const_assert!(::std::mem::size_of::<*const ::std::ffi::c_void>() <= 8);
                ::uniffi::check_remaining(buf, 8)?;
                <Self as ::uniffi::FfiConverterArc<crate::UniFfiTag>>::try_lift(
                    ::uniffi::deps::bytes::Buf::get_u64(buf) as Self::FfiType)
            }

            const TYPE_ID_META: ::uniffi::MetadataBuffer = ::uniffi::MetadataBuffer::from_code(::uniffi::metadata::codes::TYPE_INTERFACE)
                .concat_str(#mod_path)
                .concat_str(#trait_name)
                .concat_bool(true);
        }

        unsafe #lift_ref_impl_spec {
            type LiftType = ::std::sync::Arc<dyn #trait_ident>;
        }
    }
}

pub(crate) fn ffi_converter_trait_impl(trait_ident: &Ident, udl_mode: bool) -> TokenStream {
    let impl_spec = tagged_impl_header("FfiConverterArc", &quote! { dyn #trait_ident }, udl_mode);
    let lift_ref_impl_spec = tagged_impl_header("LiftRef", &quote! { dyn #trait_ident }, udl_mode);
    let name = ident_to_string(trait_ident);
    let mod_path = match mod_path() {
        Ok(p) => p,
        Err(e) => return e.into_compile_error(),
    };

    quote! {
        // All traits must be `Sync + Send`. The generated scaffolding will fail to compile
        // if they are not, but unfortunately it fails with an unactionably obscure error message.
        // By asserting the requirement explicitly, we help Rust produce a more scrutable error message
        // and thus help the user debug why the requirement isn't being met.
        uniffi::deps::static_assertions::assert_impl_all!(dyn #trait_ident: Sync, Send);

        unsafe #impl_spec {
            type FfiType = *const ::std::os::raw::c_void;

            fn lower(obj: ::std::sync::Arc<Self>) -> Self::FfiType {
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(obj)) as *const ::std::os::raw::c_void
            }

            fn try_lift(v: Self::FfiType) -> ::uniffi::Result<::std::sync::Arc<Self>> {
                let foreign_arc = ::std::boxed::Box::leak(unsafe { Box::from_raw(v as *mut ::std::sync::Arc<Self>) });
                // Take a clone for our own use.
                Ok(::std::sync::Arc::clone(foreign_arc))
            }

            fn write(obj: ::std::sync::Arc<Self>, buf: &mut Vec<u8>) {
                ::uniffi::deps::static_assertions::const_assert!(::std::mem::size_of::<*const ::std::ffi::c_void>() <= 8);
                ::uniffi::deps::bytes::BufMut::put_u64(
                    buf,
                    <Self as ::uniffi::FfiConverterArc<crate::UniFfiTag>>::lower(obj) as u64,
                );
            }

            fn try_read(buf: &mut &[u8]) -> ::uniffi::Result<::std::sync::Arc<Self>> {
                ::uniffi::deps::static_assertions::const_assert!(::std::mem::size_of::<*const ::std::ffi::c_void>() <= 8);
                ::uniffi::check_remaining(buf, 8)?;
                <Self as ::uniffi::FfiConverterArc<crate::UniFfiTag>>::try_lift(
                    ::uniffi::deps::bytes::Buf::get_u64(buf) as Self::FfiType)
            }

            const TYPE_ID_META: ::uniffi::MetadataBuffer = ::uniffi::MetadataBuffer::from_code(::uniffi::metadata::codes::TYPE_INTERFACE)
                .concat_str(#mod_path)
                .concat_str(#name)
                .concat_bool(true);
        }

        unsafe #lift_ref_impl_spec {
            type LiftType = ::std::sync::Arc<dyn #trait_ident>;
        }
    }
}

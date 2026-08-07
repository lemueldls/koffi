use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    FnArg, Ident, ImplItem, Item, ItemFn, ItemImpl, Pat, PatType, Receiver, ReceiverKind,
    ReturnType, Signature, Type, TypePath, fold::Fold, parse_macro_input, visit::Visit,
};

#[proc_macro_attribute]
pub fn export(_args: TokenStream, input: TokenStream) -> TokenStream {
    match parse_macro_input!(input as Item) {
        Item::Fn(f) => parse_function(f),
        Item::Struct(s) => TokenStream::from(quote! { #s }),
        Item::Enum(e) => TokenStream::from(quote! { #e }),
        Item::Impl(i) => parse_impl(i),
        _ => {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[koffi::export] can only be applied to a fn, a struct, an enum, or an impl block",
            )
            .to_compile_error()
            .into()
        }
    }
}

fn parse_function(input_fn: ItemFn) -> TokenStream {
    // A standalone fn can't use `self`/`Self`; finding either means the
    // attribute landed on a method instead of its `impl` block, which would
    // emit a static into the impl body. Reject it here with a good span.
    if let Some(witness) = impl_only_witness(&input_fn.sig) {
        return syn::Error::new_spanned(
            witness,
            "#[koffi::export] can't go on an individual method or associated fn, \
             `self`/`Self` only make sense inside an `impl` block. Put the \
             attribute on the surrounding `impl Type { .. }` instead, every fn \
             in it is picked up automatically.",
        )
        .to_compile_error()
        .into();
    }

    let fn_name_raw = input_fn.sig.ident.to_string();
    let symbol_ident = entry_ident(&[&fn_name_raw]);
    let entry = fn_shape_entry(&input_fn.sig, &symbol_ident, None);

    TokenStream::from(quote! {
        #input_fn

        #entry
    })
}

/// A `#[koffi::export] impl Type { ... }` block. Emitted unchanged; every
/// fn inside gets a `FnShapeRef` static with `parent: Some(Type)`, receiver
/// or not, see `fn_shape_entry`. `Type` can be a struct or an enum.
fn parse_impl(item_impl: ItemImpl) -> TokenStream {
    let self_ty = &*item_impl.self_ty;
    let self_ty_name = type_ident_fragment(self_ty);

    let entries: Vec<_> = item_impl
        .items
        .iter()
        .filter_map(|item| {
            let ImplItem::Fn(method) = item else {
                return None;
            };

            let fn_name_raw = method.sig.ident.to_string();
            let symbol_ident = entry_ident(&[&self_ty_name, &fn_name_raw]);

            Some(fn_shape_entry(&method.sig, &symbol_ident, Some(self_ty)))
        })
        .collect();

    TokenStream::from(quote! {
        #item_impl

        #( #entries )*
    })
}

/// Builds one `#[used] pub static #symbol_ident: koffi::FnShapeRef = ...;`
/// item describing `sig`. Shared by the free-function path and every fn
/// inside an `impl` block so the two can't drift apart.
///
/// `self_ty` is the enclosing `impl` block's Self type, `None` for a free
/// function. It drives two independent things:
///
/// - `FnShapeRef::parent`, always `self_ty` verbatim. Purely a
///   path/symbol-naming concern: `Payload::new(data: u16) -> Self` has a parent
///   despite taking no receiver at all.
/// - Whether `sig` takes a `self` receiver (checked from `sig` itself). If so,
///   a synthetic `self` `FnShapeParam` with `is_receiver: true` and
///   `param_type: self_ty` is prepended to `params`, since the generated Cabi
///   function takes the receiver as an ordinary by-value argument.
///
/// Any bare `Self` in the rest of the signature is substituted with
/// `self_ty`: the static is spliced in as a sibling of the `impl` block,
/// where a literal `Self` wouldn't resolve.
fn fn_shape_entry(
    sig: &Signature,
    symbol_ident: &Ident,
    self_ty: Option<&Type>,
) -> proc_macro2::TokenStream {
    let fn_name_raw = sig.ident.to_string();

    let mut param_names: Vec<String> = Vec::new();
    let mut param_types: Vec<Type> = Vec::new();
    let mut param_is_receiver: Vec<bool> = Vec::new();
    let mut param_is_mut_receiver: Vec<bool> = Vec::new();

    for arg in &sig.inputs {
        if let FnArg::Typed(PatType { pat, ty, .. }) = arg
            && let Pat::Ident(pat_ident) = &**pat
        {
            param_names.push(pat_ident.ident.to_string());
            param_types.push(resolve_self(ty, self_ty));
            param_is_receiver.push(false);
            param_is_mut_receiver.push(false);
        }
    }

    let receiver_mutability = sig.inputs.iter().find_map(|arg| {
        match arg {
            FnArg::Receiver(r) => Some(receiver_is_mut(r)),
            FnArg::Typed(_) => None,
        }
    });
    let has_receiver = receiver_mutability.is_some();
    // The receiver, if any, is always self_ty. `has_receiver` with
    // `self_ty: None` shouldn't be reachable (parse_function rejects it);
    // treat as "no receiver" rather than panicking.
    if let Some(self_ty) = has_receiver.then_some(self_ty).flatten() {
        param_names.insert(0, "self".to_string());
        param_types.insert(0, self_ty.clone());
        param_is_receiver.insert(0, true);
        param_is_mut_receiver.insert(0, receiver_mutability.unwrap_or(false));
    }

    let parent_shape = match self_ty {
        Some(ty) => {
            quote! {
                ::core::option::Option::Some(
                    ::koffi::TypeShapeRef::from_shape(<#ty as ::facet::Facet>::SHAPE)
                )
            }
        }
        None => quote! { ::core::option::Option::None },
    };

    let return_type = match &sig.output {
        ReturnType::Default => quote!(()),
        ReturnType::Type(_, ty) => {
            let ty = resolve_self(ty, self_ty);
            quote!(#ty)
        }
    };

    quote! {
        #[used]
        #[unsafe(no_mangle)]
        #[unsafe(link_section = ".koffi_fns")]
        pub static #symbol_ident: ::koffi::FnShapeRef = ::koffi::FnShapeRef {
            name: #fn_name_raw,
            params: &[
                #( ::koffi::FnShapeParam {
                    name: #param_names,
                    param_type: ::koffi::TypeShapeRef::from_shape(<#param_types as ::facet::Facet>::SHAPE),
                    is_receiver: #param_is_receiver,
                    is_mut_receiver: #param_is_mut_receiver,
                } ),*
            ],
            return_type: ::koffi::TypeShapeRef::from_shape(<#return_type as ::facet::Facet>::SHAPE),
            module_path: ::core::option::Option::Some(::core::module_path!()),
            parent: #parent_shape,
        };
    }
}

/// Whether a receiver argument is `&mut self` (or `mut self`). syn 3 stashes
/// the `mut` of `&mut self` in `ReceiverKind::Reference`, leaving the
/// top-level `Receiver::mutability` field as `None`; `mut self` keeps it in
/// the top-level field. Both matter, so check both places.
fn receiver_is_mut(r: &Receiver) -> bool {
    match &r.kind {
        ReceiverKind::Reference(_, _, mutability) => mutability.is_some(),
        _ => r.mutability.is_some(),
    }
}

/// Collision-resistant static ident for a `FnShapeRef`:///
/// `__KOFFI_FN_<PART>_..._ENTRY`, uppercased. Takes multiple parts so a
/// method's ident folds in its receiver type name
/// (`__KOFFI_FN_PAYLOAD_GET_ENTRY`).
fn entry_ident(parts: &[&str]) -> Ident {
    let joined = parts
        .iter()
        .map(|p| p.trim_start_matches("r#").to_uppercase())
        .collect::<Vec<_>>()
        .join("_");

    quote::format_ident!("__KOFFI_FN_{}_ENTRY", joined)
}

/// Best-effort identifier fragment for a `Self` type, for generated static
/// idents. Falls back to a fixed name for anything exotic (tuple types,
/// generics-heavy paths); those still compile, they just share a less
/// descriptive prefix.
fn type_ident_fragment(ty: &Type) -> String {
    if let Type::Path(type_path) = ty
        && let Some(seg) = type_path.path.segments.last()
    {
        return seg.ident.to_string();
    }

    "TYPE".to_string()
}

/// Rewrites every bare `Self` inside `ty` to `self_ty`, leaving everything
/// else untouched. A no-op when `self_ty` is `None`: a free function can't
/// mention `Self` in valid Rust anyway.
///
/// Only a bare `Self` path (one segment, no generic args, no qualified
/// `Self::AssocType`) is matched; `Self::AssocType` needs actually
/// resolving the associated type, not a textual substitution.
fn resolve_self(ty: &Type, self_ty: Option<&Type>) -> Type {
    match self_ty {
        Some(self_ty) => ResolveSelf { self_ty }.fold_type(ty.clone()),
        None => ty.clone(),
    }
}

struct ResolveSelf<'a> {
    self_ty: &'a Type,
}

impl Fold for ResolveSelf<'_> {
    fn fold_type(&mut self, ty: Type) -> Type {
        if is_bare_self(&ty) {
            return self.self_ty.clone();
        }

        syn::fold::fold_type(self, ty)
    }
}

/// Does `ty` mention a bare `Self` anywhere inside it (`Self`, `&Self`,
/// `Option<Self>`, a tuple element, and so on)? Same `Self::AssocType`
/// caveat as `resolve_self`.
fn type_references_self(ty: &Type) -> bool {
    let mut finder = SelfFinder { found: false };
    finder.visit_type(ty);
    finder.found
}

struct SelfFinder {
    found: bool,
}

impl<'ast> Visit<'ast> for SelfFinder {
    fn visit_type(&mut self, ty: &'ast Type) {
        if is_bare_self(ty) {
            self.found = true;
            return;
        }

        syn::visit::visit_type(self, ty);
    }
}

fn is_bare_self(ty: &Type) -> bool {
    matches!(ty, Type::Path(TypePath { qself: None, path, .. }) if path.is_ident("Self"))
}

/// A span-carrying witness if `sig` could only have compiled inside an///
/// `impl`/`trait` block: it takes a `self` receiver, or `Self` shows up in
/// a param or the return type. Used by `parse_function` to reject
/// `#[koffi::export]` applied directly to a method or associated fn. Both
/// signals are unambiguous, not heuristics: neither `self` nor `Self`
/// type-checks outside an impl/trait regardless of koffi.
fn impl_only_witness(sig: &Signature) -> Option<proc_macro2::TokenStream> {
    for arg in &sig.inputs {
        match arg {
            FnArg::Receiver(r) => return Some(r.to_token_stream()),
            FnArg::Typed(PatType { ty, .. }) if type_references_self(ty) => {
                return Some(ty.to_token_stream());
            }
            FnArg::Typed(_) => {}
        }
    }

    if let ReturnType::Type(_, ty) = &sig.output
        && type_references_self(ty)
    {
        return Some(ty.to_token_stream());
    }

    None
}

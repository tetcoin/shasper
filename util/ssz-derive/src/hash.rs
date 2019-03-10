use proc_macro2::TokenStream;
use syn::{
	Data, Field, Fields, Ident, Index,
	punctuated::Punctuated,
	spanned::Spanned,
	token::Comma,
};

type FieldsList = Punctuated<Field, Comma>;

fn truncated(attrs: &[syn::Attribute]) -> bool {
	attrs.iter().any(|attr| {
		attr.path.segments.first().map(|pair| {
			let seg = pair.value();

			if seg.ident == Ident::new("ssz", seg.ident.span()) {
				assert_eq!(attr.path.segments.len(), 1);

				let meta = attr.interpret_meta();
				if let Some(syn::Meta::List(ref l)) = meta {
					if let syn::NestedMeta::Meta(syn::Meta::Word(ref w)) = l.nested.last().unwrap().value() {
						assert_eq!(w, &Ident::new("truncate", w.span()));
						true
					} else {
						panic!("Invalid syntax for `ssz` attribute: Expected truncated.");
					}
				} else {
					panic!("Invalid syntax for `ssz` attribute: Expected truncated.");
				}
			} else {
				false
			}
		}).unwrap_or(false)
	})
}

fn encode_fields<F>(
	dest: &TokenStream,
	generic_param: &TokenStream,
	fields: &FieldsList,
	field_name: F,
	skip_truncated: bool,
) -> TokenStream where
	F: Fn(usize, &Option<Ident>) -> TokenStream,
{
	let fields: Vec<_> = fields.iter().collect();

	let recurse = fields.iter().enumerate().map(|(i, f)| {
		let truncate = if skip_truncated {
			truncated(&f.attrs)
		} else {
			false
		};
		let field = field_name(i, &f.ident);

		if truncate {
			quote! { (); }
		} else {
			quote_spanned! { f.span() =>
							 #dest.append(&mut ::ssz::Hashable::hash::< #generic_param >(#field).as_ref().to_vec());
			}
		}
	});

	quote! {
		#( #recurse )*
	}
}

pub fn quote(data: &Data, self_: &TokenStream, dest: &TokenStream, generic_param: &TokenStream, skip_truncated: bool) -> TokenStream {
	let call_site = proc_macro2::Span::call_site();
	match *data {
		Data::Struct(ref data) => match data.fields {
			Fields::Named(ref fields) => encode_fields(
				dest,
				generic_param,
				&fields.named,
				|_, name| quote_spanned!(call_site => &#self_.#name),
				skip_truncated,
			),
			Fields::Unnamed(ref fields) => encode_fields(
				dest,
				generic_param,
				&fields.unnamed,
				|i, _| {
					let index = Index { index: i as u32, span: call_site };
					quote_spanned!(call_site => &#self_.#index)
				},
				skip_truncated,
			),
			Fields::Unit => quote! { (); },
		},
		Data::Enum(_) => panic!("Enum types are not supported."),
		Data::Union(_) => panic!("Union types are not supported."),
	}
}

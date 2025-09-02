use proc_macro::TokenStream;
use syn::{parse_macro_input, punctuated::Punctuated, Item, ItemEnum, ItemStruct, Token};

fn derive_item_struct(item: ItemStruct) -> TokenStream {
    let ident = item.ident.to_string();

    let mut pack = format!(
        "impl TSerialize for {ident} {{
            fn serialize(&self, w: &mut impl std::io::Write) {{
    "
    );

    for fld in &item.fields {
        pack = format!(
            "{pack}\n self.{}.serialize(w);",
            fld.ident.as_ref().unwrap()
        );
    }

    pack = format!("{pack} \n }} \n }}");

    pack = format!(
        "{pack}\n impl TDeserialize for {ident} {{
            fn deserialize(r: &mut impl std::io::Read) -> Self {{
                Self {{
    "
    );
    for fld in &item.fields {
        pack = format!("{pack}\n {}: deserialize(r),", fld.ident.as_ref().unwrap());
    }

    pack = format!("{pack}  }} \n }} \n }}");

    pack.parse().unwrap()
}

fn derive_serialize_item_enum(item: ItemEnum) -> TokenStream {
    let ident = item.ident.to_string();

    let mut pack = format!(
        "impl TSerialize for {ident} {{
            fn serialize(&self, w: &mut impl std::io::Write) {{
                match self {{
    "
    );

    for (f_i, field) in item.variants.iter().enumerate() {
        let mut content_info = String::new();

        let mut extra = String::new();

        match &field.fields {
            syn::Fields::Named(fields_named) => {
                for named in &fields_named.named {
                    content_info = format!(
                        "{content_info} \n {}.serialize(w);",
                        named.ident.as_ref().unwrap()
                    )
                }
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                extra = "(".into();
                for (unnamed_i, _unnamed) in fields_unnamed.unnamed.iter().enumerate() {
                    extra = format!("{extra} v{unnamed_i},");
                    content_info = format!("{content_info} \n v{unnamed_i}.serialize(w);")
                }
                extra = format!("{extra})");
            }
            syn::Fields::Unit => {
                // do nothing!
            }
        }

        let this = format!(
            "{ident}::{} {extra} => {{
            let index : u32 = {f_i};
            index.serialize(w);
            {content_info}
         }}",
            field.ident
        );

        pack = format!("{pack} \n {this}");
    }

    pack = format!("{pack}  }} \n }} \n }}");

    // now for the deserialize part ===========================================

    pack = format!(
        "{pack}\n impl TDeserialize for {ident} {{
            fn deserialize(r: &mut impl std::io::Read) -> Self {{
                let index = u32::deserialize(r);
                match index {{
    "
    );

    for (f_i, field) in item.variants.iter().enumerate() {
        let mut content_info = String::new();

        match &field.fields {
            syn::Fields::Named(fields_named) => {
                content_info = "{".into();
                for named in &fields_named.named {
                    content_info = format!(
                        "{content_info} \n {}: deserialize(r), ",
                        named.ident.as_ref().unwrap()
                    )
                }
                content_info = format!("{content_info} }}");
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                content_info = "(".into();
                for _ in &fields_unnamed.unnamed {
                    content_info = format!("{content_info} \n deserialize(r), ")
                }
                content_info = format!("{content_info})");
            }
            syn::Fields::Unit => {
                // do nothing!
            }
        }

        let this = format!(
            "{f_i} => {{
                return {ident}::{} {content_info}
            }}",
            field.ident
        );

        pack = format!("{pack} \n {this}");
    }

    pack = format!(
        "{pack} \n
        _ => {{eprintln!(\"Unknown enum value {{index}}!\"); unreachable!();}}
    }} \n }} \n }}"
    );

    pack.parse().expect("creating derive")
}

#[proc_macro_derive(TSerialize)]
pub fn derive_answer_fn(item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);

    match item {
        Item::Struct(item_struct) => derive_item_struct(item_struct),
        Item::Enum(item_enum) => derive_serialize_item_enum(item_enum),
        _ => panic!("Must be used on an enum or struct only"),
    }
}

// =============================================================================

struct FWInput {
    ident: syn::Ident,
    types: Punctuated<syn::Ident, Token![,]>,
}

impl syn::parse::Parse for FWInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse the identifier (e.g., the name of the struct)
        let ident: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?; // Expect a comma after the identifier

        // Parse the list of types (comma-separated)
        let types = Punctuated::<syn::Ident, Token![,]>::parse_terminated(input)?;

        Ok(FWInput { ident, types })
    }
}

#[proc_macro]
pub fn serde_enum_framework(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as FWInput);

    let name = input.ident;

    let variant_count = input.types.iter().count();

    const U8_LIMIT: usize = u8::MAX as usize;
    const U16_LIMIT: usize = u16::MAX as usize;
    const U32_LIMIT: usize = u32::MAX as usize;

    let variant_type = match variant_count {
        0..U8_LIMIT => "u8",
        U8_LIMIT..U16_LIMIT => "u16",
        U16_LIMIT..U32_LIMIT => "u32",
        _ => panic!("unable to handle such large variants at this time"),
    };

    assert!(
        variant_count < u8::MAX as usize,
        "only works for small variants at the moment (<= 256)"
    );

    let mut ret =
        format!("pub trait Encode{name} {{ fn encode_to(&self, w: &mut impl std::io::Write); }}");

    for (i, variant) in input.types.iter().enumerate() {
        let variant_name = &variant;
        let func_name = format!("impl Encode{name} for {variant_name}{{ 
        fn encode_to(&self, w: &mut impl std::io::Write) {{ {i}{variant_type}.serialize(w); self.serialize(w); }} }}\n");

        ret.push_str(&func_name);
    }

    // impl decode handler

    ret.push_str(&format!("pub trait Decode{name} {{"));

    for variant in input.types.iter() {
        let variant_name = &variant;
        let variant_name_lower = variant_name.to_string().to_lowercase();
        let func_name =
            format!("fn handle_{variant_name_lower} (&mut self, item: {variant_name});\n");

        ret.push_str(&func_name);
    }

    ret.push_str("}");

    // impl decode function

    ret.push_str(&format!(
        "#[allow(non_snake_case)] pub fn decode_{name}(r: &mut impl std::io::Read, handler: &mut impl Decode{name}) {{\n
    let id = {variant_type}::deserialize(r);
    match id {{\n
    "
    ));

    for (i, variant) in input.types.iter().enumerate() {
        let variant_name = &variant;
        let variant_name_lower = variant_name.to_string().to_lowercase();
        let func_name = format!("{i} => handler.handle_{variant_name_lower}(deserialize(r)),\n");

        ret.push_str(&func_name);
    }

    ret.push_str("_ => unreachable!(\"out of bounds\"), } }");

    ret.parse().unwrap()
}

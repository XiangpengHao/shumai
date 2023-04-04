extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;
use proc_macro::TokenStream;
use syn::{parse_macro_input, GenericArgument};

#[proc_macro_attribute]
pub fn config(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as syn::AttributeArgs)
        .first()
        .expect("Benchmark file must be annotated with #[config(path = \"/path/to/file.toml\")]")
        .clone();
    let file_path = get_config_file_path(&args)
        .expect("Benchmark file must be annotated with #[config(path = \"/path/to/file.toml\")]");

    let ty: syn::Item = syn::parse_macro_input!(input as syn::Item);

    let item_struct = if let syn::Item::Struct(m) = ty {
        m
    } else {
        panic!("config attribute must be applied to a Struct");
    };

    let name = &item_struct.ident;
    let matrix_name = gen_matrix_name(name);

    let fields = if let syn::Fields::Named(syn::FieldsNamed { ref named, .. }) = item_struct.fields
    {
        named
    } else {
        panic!("config attribute must be applied to a Struct with named fields");
    };

    let config_fields = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;

        if name.as_ref().unwrap() == "threads" && is_matrix_field(f) {
            panic!("threads can't be marked as matrix, it's matrix by definition");
        }
        if is_matrix_field(f) {
            // // If the type is Option, return Option<Vec<ty>>; otherwise return Vec<ty>
            if let Some(t) = get_optional_inner_type(ty) {
                quote! {#name: std::option::Option<std::vec::Vec<#t>>}
            } else {
                quote! {#name: std::vec::Vec<#ty>}
            }
        } else {
            quote! {#name: #ty}
        }
    });

    let methods = gen_methods(fields, 0, name);
    let dummy_struct_name = syn::Ident::new(&format!("{name}DummyStruct"), name.span());
    let expanded = quote! {
        #[derive(Debug, shumai::__dep::serde::Deserialize)]
        pub struct #matrix_name {
            #(#config_fields, )*
        }

        impl #matrix_name {
            pub fn unfold(&self) -> std::vec::Vec<#name> {
                let mut configs: std::vec::Vec<#name> = std::vec::Vec::new();

                #methods

                configs
            }
        }

        #[derive(Debug, Clone, shumai::ShumaiConfig, shumai::__dep::serde::Serialize, shumai::__dep::serde::Deserialize)]
        #item_struct

        #[derive(shumai::__dep::serde::Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct #dummy_struct_name {
            #name: std::option::Option<std::vec::Vec<#matrix_name>>,
        }

        impl #name {
            #[allow(non_snake_case)]
            pub fn load() -> std::option::Option<std::vec::Vec<#name>> {
                let contents = std::fs::read_to_string(#file_path).expect(&format!("failed to read the benchmark config file at {}", #file_path));
                let configs = shumai::__dep::toml::from_str::<#dummy_struct_name>(&contents).expect(&format!("failed to parse the benchmark config file at {}", #file_path));

                let configs = configs.#name?;

                let mut expanded = std::vec::Vec::new();
                for b in configs.iter() {
                    expanded.extend(b.unfold());
                }


                match std::env::var("SHUMAI_FILTER") {
                    Ok(filter) => {
                        let regex_filter =
                            shumai::__dep::regex::Regex::new(filter.as_ref())
                            .expect(&format!("Filter {} from env `SHUMAI_FILTER` is not a valid regex expression!", filter));
                        let configs: std::vec::Vec<_> = expanded.into_iter().filter(|c| regex_filter.is_match(&c.name)).collect();
                        Some(configs)
                    },
                    Err(_) => {
                        Some(expanded)
                    }
                }
            }
        }

        impl shumai::BenchConfig for #name {
            fn name(&self) -> &String {
                &self.name
            }

            fn thread(&self) -> &[usize] {
                &self.threads
            }

            fn bench_sec(&self) -> usize {
                self.time
            }
        }
    };

    // eprintln!("{}", expanded);
    expanded.into()
}

#[proc_macro_derive(ShumaiConfig, attributes(matrix))]
pub fn derive_bench_config(_input: TokenStream) -> TokenStream {
    quote!().into()
}

fn gen_matrix_name(name: &syn::Ident) -> syn::Ident {
    let gen_name = format!("{name}Matrix");
    syn::Ident::new(&gen_name, name.span())
}

fn gen_methods(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
    current: usize,
    origin_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    if current == fields.len() {
        let name_prefix = origin_name.to_string().to_ascii_lowercase();
        let mut name_gen = quote! {
            let mut name_lit = format!("{}-{}", #name_prefix, name.clone());
        };
        for f in fields {
            let f_name = &f.ident;
            if is_matrix_field(f) {
                if get_optional_inner_type(&f.ty).is_some() {
                    name_gen = quote! {
                        #name_gen
                        if let Some(t_v) = &self.#f_name {
                            if t_v.len() > 1 {
                                name_lit = format!("{}-{:?}", name_lit, #f_name);
                            }
                        }
                    }
                } else {
                    name_gen = quote! {
                        #name_gen
                        if self.#f_name.len() > 1 {
                            name_lit = format!("{}-{:?}", name_lit, #f_name);
                        }
                    }
                }
            }
        }
        let assign_fields = fields.iter().map(|f| {
            let name = &f.ident;
            // We skip the `name` field because it's handled separately
            if name.as_ref().unwrap() == "name" {
                quote! {}
            } else {
                quote! {
                    #name: #name.clone(),
                }
            }
        });

        return quote! {
            #name_gen
            configs.push(#origin_name {
                name: name_lit,
                #(#assign_fields)*
            });
        };
    }

    let inner = gen_methods(fields, current + 1, origin_name);

    let current = &fields[current];
    let name = &current.ident;

    if is_matrix_field(current) {
        if get_optional_inner_type(&current.ty).is_some() {
            quote! {
                if let Some(#name) = &self.#name {
                    for i in #name.iter() {
                        let #name = Some(i.clone());
                        #inner
                    }
                }else{
                    let #name = None;
                    #inner
                }
            }
        } else {
            quote! {
                for i in self.#name.iter() {
                    let #name = i.clone();
                    #inner
                }
            }
        }
    } else {
        quote! {
            let #name = self.#name.clone();
            #inner
        }
    }
}

fn is_matrix_field(f: &syn::Field) -> bool {
    for attr in &f.attrs {
        if attr.path.segments.len() == 1 && attr.path.segments[0].ident == "matrix" {
            return true;
        }
    }
    false
}

fn get_optional_inner_type(ty: &syn::Type) -> Option<&GenericArgument> {
    if let syn::Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    }) = ty
    {
        if segments.len() == 1 {
            let segment = segments.first().unwrap();
            if segment.ident == "Option" {
                let option_inner = &segment.arguments;
                match option_inner {
                    syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                        args,
                        ..
                    }) => {
                        let ty = args.first().unwrap();
                        return Some(ty);
                    }
                    _ => panic!("Option must be used with angle bracketed generic arguments"),
                }
            }
        }
    }
    None
}

fn get_config_file_path(meta: &syn::NestedMeta) -> Option<syn::LitStr> {
    let meta = if let syn::NestedMeta::Meta(m) = meta {
        m
    } else {
        return None;
    };

    let name_value = if let syn::Meta::NameValue(v) = meta {
        v
    } else {
        return None;
    };

    if name_value.path.segments[0].ident != "path" {
        return None;
    }

    let v = if let syn::Lit::Str(v) = name_value.lit.clone() {
        v
    } else {
        return None;
    };

    Some(v)
}

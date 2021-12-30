extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

/// Generate helper structs/functions for benchmark configs
/// For example, given the following struct:
/// ```ignore
/// #[derive(Debug, BenchConfig, Deserialize, Serialize)]
/// pub struct TableConfig {
///     name: String,
///     #[matrix]
///     threads: usize,
/// }
/// ```
/// It adds a matrix struct:
/// ```ignore
/// pub struct TableConfigMatrix {
///     name: String,
///     threads: Vec<usize>
/// }
/// ```
/// The `TableConfigMatrix` allows you to write concise config in .toml files, i.e.
/// ```toml
/// [[table]]
/// name = "dist"
/// threads = [1, 2, 4, 8, 16, 24]
/// ```
/// It also adds the following helper functions:
/// ```ignore
/// impl TableConfigMatrix {
///     fn unfold(&self) -> Vec<TableConfig>;
///     pub fn is_match(&self, filter: &regex::Regex) -> bool;
/// }
/// ```
#[proc_macro_derive(ShumaiConfig, attributes(matrix))]
pub fn derive_bench_config(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = ast.ident;

    let mident = gen_matrix_name(&name);
    let lower_name = gen_lower_ident(&name);

    let fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named
    } else {
        unimplemented!();
    };

    let config_fields = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        if name.as_ref().unwrap() == "threads" && is_matrix_field(f) {
            panic!("threads can't be marked as matrix, it's matrix by definition");
        }
        if is_matrix_field(f) {
            quote! { #name: std::vec::Vec<#ty> }
        } else {
            quote! { #name: #ty }
        }
    });

    let methods = gen_methods(fields, 0, &name);

    let expanded = quote! {
        #[derive(Debug, shumai::__dep::serde::Deserialize)]
        pub struct #mident {
            #(#config_fields, )*
        }

        impl #mident {
            pub fn unfold(&self) -> std::vec::Vec<#name> {
                let mut configs: std::vec::Vec<#name> = std::vec::Vec::new();

                #methods

                configs
            }
        }

        impl #name {
            pub fn is_match(&self, filter: &shumai::__dep::regex::Regex) -> bool {
                filter.is_match(&self.name)
            }

            pub fn load() -> std::option::Option<std::vec::Vec<#name>>{
                use super::BenchRootConfig;
                BenchRootConfig::load().#lower_name()
            }

            pub fn from_config(path: impl AsRef<std::path::Path>) -> std::option::Option<std::vec::Vec<#name>> {
                use super::BenchRootConfig;
                BenchRootConfig::load_config(path.as_ref()).#lower_name()
            }
        }

        unsafe impl Sync for #name {}
        unsafe impl Send for #name {}

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

    expanded.into()
}

#[proc_macro_attribute]
pub fn bench_config(args: TokenStream, input: TokenStream) -> TokenStream {
    assert!(args.is_empty());
    let mut out = input.clone();
    let ty: syn::Item = syn::parse_macro_input!(input as syn::Item);

    let mut all_configs = Vec::new();

    let item_mod = if let syn::Item::Mod(m) = ty {
        m
    } else {
        panic!("config attribute must be applied to mod");
    };
    let mod_items = item_mod.content.expect("mod can't be empty").1;
    for item in mod_items {
        if let syn::Item::Struct(conf_s) = item {
            all_configs.push(conf_s.ident);
        }
    }
    let mod_name = item_mod.ident;

    let root_fields = all_configs.iter().map(|f| {
        let matrix_name = gen_matrix_name(f);
        let lower_name = gen_lower_ident(f);
        quote! {
            #lower_name: std::option::Option<std::vec::Vec<#matrix_name>>
        }
    });

    let field_functions = all_configs.iter().map(|f| {
        let lower_name = gen_lower_ident(f);
        quote! {
            pub fn #lower_name(&self) -> std::option::Option<std::vec::Vec<#f>>{
                let mut configs = std::vec::Vec::new();
                for b in self.#lower_name.as_ref()?.iter(){
                    configs.extend(b.unfold());
                }

                // println!("{:?}", configs);
                // let args: std::vec::Vec<String> = std::env::args().collect();


                // let default_filter = ".*".to_string();
                // let filter_str = args.get(1).unwrap_or_else(|| &default_filter);
                // let bench_filter =
                //    shumai::__dep::regex::Regex::new(&filter_str).expect("failed to parse the benchmark filter into regex expression!");

                // let configs: std::vec::Vec<_> = configs.into_iter().filter(|c| c.is_match(&bench_filter)).collect();

                Some(configs)
            }
        }
    });

    let field_import = all_configs.iter().map(|f| {
        let matrix_name = gen_matrix_name(f);
        quote! {
            use self::#mod_name::{#matrix_name, #f};
        }
    });

    let root_config = quote! {

        #(#field_import)*

        #[derive(shumai::__dep::serde::Deserialize, Debug)]
        struct BenchRootConfig {
            #(#root_fields,)*
        }

        impl BenchRootConfig {
            fn load_config(path: &std::path::Path)-> Self {
                let contents = std::fs::read_to_string(path).unwrap();
                let configs: BenchRootConfig =
                    shumai::__dep::toml::from_str(&contents).expect("Unable to parse the toml file");
                configs
            }

            fn load() -> Self {
                BenchRootConfig::load_config(std::path::Path::new("benchmark.toml"))
            }

            #(#field_functions)*
        }
    };

    out.extend(TokenStream::from(root_config));

    out
}

fn gen_matrix_name(name: &syn::Ident) -> syn::Ident {
    let gen_name = format!("{}Matrix", name);
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
                name_gen = quote! {
                    #name_gen
                    if self.#f_name.len() > 1 {
                        name_lit = format!("{}-{}", name_lit, #f_name);
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
        quote! {
            for i in self.#name.iter() {
                let #name = *i;
                #inner
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

fn gen_lower_ident(f: &syn::Ident) -> syn::Ident {
    let lower_name = f.to_string().to_ascii_lowercase();
    syn::Ident::new(&lower_name, f.span())
}

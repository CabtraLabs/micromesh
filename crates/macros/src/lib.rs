use proc_macro::TokenStream;
use quote::quote;
use heck::ToUpperCamelCase;
use syn::{parse_macro_input, ItemTrait, FnArg, PatType, ReturnType, parse_quote};

#[proc_macro_attribute]
pub fn remote_trait(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemTrait);
    let trait_name = &input.ident;

    let params_enum_name = syn::Ident::new(&format!("{}_params", trait_name).to_upper_camel_case(), trait_name.span());
    let result_enum_name = syn::Ident::new(&format!("{}_result", trait_name).to_upper_camel_case(), trait_name.span());
    let server_struct_name = syn::Ident::new(&format!("{}_rpc_server", trait_name).to_upper_camel_case(), trait_name.span());    
    let client_struct_name = syn::Ident::new(&format!("{}_rpc_client", trait_name).to_upper_camel_case(), trait_name.span());
    // input.supertraits.push(parse_quote!(Sized + Clone + Send + Sync));
    input.supertraits.push(parse_quote!(Sized));
    input.supertraits.push(parse_quote!(Clone));
    input.supertraits.push(parse_quote!(Send));
    input.supertraits.push(parse_quote!(Sync));

    let mut param_variants = vec![];
    let mut result_variants = vec![];
    let mut rpc_arms = vec![];
    let mut client_impls = vec![];

    for item in &mut input.items {
        if let syn::TraitItem::Fn(m) = item {
            let method_name = &m.sig.ident;
            let variant_name = syn::Ident::new(&method_name.to_string().to_upper_camel_case(), method_name.span());

            m.sig.inputs.insert(1, parse_quote!(context: std::sync::Arc<Self::Context>));

            // 参数类型列表
            let param_types: Vec<_> = m.sig.inputs.iter().skip(2).map(|arg| {
                if let FnArg::Typed(PatType { ty, .. }) = arg {
                    ty
                } else {
                    panic!("Unexpected receiver")
                }
            }).collect();

            // 枚举参数分支
            param_variants.push(quote! {
                #variant_name(#(#param_types),*)
            });

            // 返回值
            let ret_type = match &m.sig.output {
                ReturnType::Default => quote! { () },
                ReturnType::Type(_, ty) => quote! { #ty },
            };
            result_variants.push(quote! {
                #variant_name(#ret_type)
            });

            // rpc match 分支
            let param_names: Vec<_> = (0..param_types.len())
                .map(|i| syn::Ident::new(&format!("p{}", i), proc_macro2::Span::call_site()))
                .collect();

            rpc_arms.push(quote! {
                #params_enum_name::#variant_name(#(#param_names),*) => {
                    #result_enum_name::#variant_name(self.#method_name(context, #(#param_names),*).await)
                }
            });

            client_impls.push(quote! {
                async fn #method_name(context, #(#param_types),*) -> #variant_name(#ret_type) {

                }
            });
        }
    }

    let lowercase_trait_name = trait_name.to_string().to_lowercase().replace("trait", "");

    input.attrs.push(parse_quote!(#[async_trait::async_trait]));

    input.items.insert(0, parse_quote!( 
        async fn __rpc_call(&self,context: std::sync::Arc<Self::Context>, params: #params_enum_name) -> #result_enum_name
        {
            match params {
                #(#rpc_arms),*
            }
        }
    ));

    input.items.insert(0, parse_quote!( fn name(&self) -> &str {
        #lowercase_trait_name
    }));

    input.items.insert(0, parse_quote!(type Context: crate::app::ContextTrait + Send + Unpin + Sync + 'static; ));
    
    let expanded = quote! {

        #input

        #[derive(Debug, bitcode::Encode, bitcode::Decode, serde::Serialize, serde::Deserialize)]
        pub enum #params_enum_name {
            #(#param_variants),*
        }
        #[derive(Debug, bitcode::Encode, bitcode::Decode, serde::Serialize, serde::Deserialize)]
        pub enum #result_enum_name {
            #(#result_variants),*
        }

        #[derive(Debug, Clone)]
        pub struct #server_struct_name<T: #trait_name >(pub T);

        #[async_trait::async_trait]
        impl<T: #trait_name > crate::app::RpcTrait for #server_struct_name<T> {
            type Context = T::Context;
            type Params = #params_enum_name;
            type Result = #result_enum_name;

            fn name(&self) -> &str {
                self.0.name()
            }

            async fn rpc_call(&self, context: std::sync::Arc<Self::Context>, params: Self::Params) -> Self::Result {
                self.0.__rpc_call(context, params).await
            }
        }

        #[derive(Debug, Clone)]
        pub struct #client_struct_name;

        /*#[async_trait::async_trait]
        impl #trait_name for #client_struct_name{
            #(#client_impls),*
        }*/

    };

    TokenStream::from(expanded)
}

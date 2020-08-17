use crate::{
    common::*,
    path::{TraitBoundsVar, TypeVar},
    scope::{Scope, TraitBoundDict},
};

#[derive(Debug)]
struct FnEnv {
    scope: Scope,
}

#[derive(Debug)]
struct FnOutput {
    output_ty: TokenStream,
    env: FnEnv,
}

pub fn translate_items(items: &[Item]) -> syn::Result<TokenStream> {
    let tokens_vec: Vec<_> = items
        .into_iter()
        .map(|item| {
            let tokens = match item {
                Item::Enum(enum_) => translate_enum(&enum_)?,
                Item::Fn(fn_) => {
                    let ItemFn { sig, block, .. } = fn_;
                    translate_fn(sig, block, None)?
                }
                Item::Struct(struct_) => translate_struct(&struct_)?,
                Item::Impl(impl_) => translate_impl(&impl_)?,
                _ => {
                    return Err(Error::new(item.span(), "unsupported item kind"));
                }
            };
            Ok(tokens)
        })
        .try_collect()?;

    let expanded = quote! {
        #(#tokens_vec)*
    };

    eprintln!("{}", expanded);

    Ok(expanded)
}

fn translate_struct(_struct_: &ItemStruct) -> syn::Result<TokenStream> {
    todo!();
}

fn translate_enum(enum_: &ItemEnum) -> syn::Result<TokenStream> {
    let ItemEnum {
        vis,
        ident: trait_name,
        variants,
        ..
    } = enum_;

    let types: Vec<_> = variants
        .iter()
        .map(|variant| -> syn::Result<_> {
            let Variant {
                ident: type_name,
                fields,
                ..
            } = variant;

            let generics = match fields {
                Fields::Unit => vec![],
                Fields::Unnamed(unnamed) => {
                    let generics: Vec<_> = unnamed
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(index, field)| -> syn::Result<_> {
                            let Field { ty, .. } = field;
                            let generic_name = format_ident!("T{}", index);
                            let trait_bounds = ty_to_trait_bounds(ty)?;

                            Ok((generic_name, trait_bounds))
                        })
                        .try_collect()?;

                    generics
                }
                Fields::Named(named) => {
                    let generics: Vec<_> = named
                        .named
                        .iter()
                        .map(|field| -> syn::Result<_> {
                            let Field { ident, ty, .. } = field;
                            let generic_name = ident.to_owned().unwrap();
                            let trait_bounds = ty_to_trait_bounds(ty)?;

                            Ok((generic_name.to_owned(), trait_bounds))
                        })
                        .try_collect()?;

                    generics
                }
            };

            Ok((type_name, generics))
        })
        .try_collect()?;

    let impls = {
        let items: Vec<_> = types
            .into_iter()
            .map(|(type_name, generics)| {
                let generic_args = {
                    let names: Vec<_> = generics.iter().map(|(name, _)| name).collect();
                    quote! {
                        #(#names),*
                    }
                };

                let where_clause = {
                    let bounds: Vec<_> = generics
                        .iter()
                        .filter_map(|(name, bounds)| {
                            if bounds.is_empty() {
                                None
                            } else {
                                Some(quote! {
                                    #name: #(#bounds)+*
                                })
                            }
                        })
                        .collect();

                    quote! {
                        #(#bounds),*
                    }
                };

                quote! {
                    #vis struct #type_name<#generic_args>
                    where
                        #where_clause
                    {
                        _phantom: ::core::marker::PhantomData<#generic_args>
                    }

                    impl<#generic_args> #trait_name for #type_name<#generic_args>
                    where
                        #where_clause
                    {}
                }
            })
            .collect();

        quote! {
            #(#items)*
        }
    };

    let expanded = quote! {
        #vis trait #trait_name {}

        #impls
    };

    Ok(expanded)
}

fn translate_impl(impl_: &ItemImpl) -> syn::Result<TokenStream> {
    let ItemImpl {
        trait_,
        self_ty,
        items,
        ..
    } = impl_;

    if trait_.is_some() {
        return Err(Error::new(
            impl_.span(),
            r#""for Trait" clause is not supported"#,
        ));
    }

    let trait_path = match &**self_ty {
        Type::Path(type_path) => &type_path.path,
        _ => return Err(Error::new(self_ty.span(), "unsupported type kind")),
    };

    let _: Vec<_> = items
        .iter()
        .map(|item| -> syn::Result<_> {
            match item {
                ImplItem::Type(type_) => {
                    let ImplItemType {
                        ident: type_name,
                        ty,
                        ..
                    } = type_;
                    todo!();
                }
                ImplItem::Method(method) => {
                    let ImplItemMethod { sig, block, .. } = method;
                    translate_fn(sig, block, Some(trait_path))?
                }
                _ => {
                    return Err(Error::new(item.span(), "unsupported item kind"));
                }
            };

            Ok(())
        })
        .try_collect()?;

    let expanded = quote! {};
    Ok(expanded)
}

fn translate_fn(
    sig: &Signature,
    block: &Block,
    self_trait_opt: Option<&Path>,
) -> syn::Result<TokenStream> {
    let Signature {
        ident: trait_op_name,
        generics,
        inputs,
        output,
        ..
    } = sig;

    // create root scope
    let scope = Scope::new();

    // TODO
    match (self_trait_opt, inputs.first()) {
        (
            Some(self_trait),
            Some(FnArg::Receiver(Receiver {
                reference: None, ..
            })),
        ) => {
            // TODO
        }
        _ => {
            return Err(Error::new(
                sig.span(),
                r#"methods in impl block must place "self" at the first argument"#,
            ))
        }
    };

    // turn function generics into initial quantifiers
    {
        // function generics (ident, [path])
        let generic_args: Vec<(&Ident, _)> = generics
            .params
            .iter()
            .map(|param| match param {
                GenericParam::Type(TypeParam { ident, bounds, .. }) => Ok((ident, bounds)),
                GenericParam::Lifetime(lifetime) => {
                    Err(Error::new(lifetime.span(), "lifetime is not allowed"))
                }
                GenericParam::Const(const_) => {
                    Err(Error::new(const_.span(), "const generics is not allowed"))
                }
            })
            .try_collect()?;

        // create initial quantifiers
        for (ident, _paths) in generic_args.iter().cloned() {
            scope.insert_initial_quantifier(ident.to_owned());
        }

        // add trait bounds for initial quantifiers
        for (ident, bounds) in generic_args.iter().cloned() {
            let predicate = TypeVar::from(ident);
            let bounds = TraitBoundsVar::from_scoped_type_param_bounds(&bounds, &scope)?;
            scope.insert_trait_bounds(predicate, bounds);
        }
    }

    // turn function arguments into trait bounds
    {
        // get function arguments (type, trait bounds)
        let fn_args: Vec<(TypeVar, TraitBoundsVar)> = inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Typed(pat_type) => Some(pat_type),
                FnArg::Receiver(_) => None,
            })
            .map(|PatType { pat, ty, .. }| -> syn::Result<_> {
                let type_ = TypeVar::from_scoped_pat(&**pat, &scope)?;
                let trait_bounds = TraitBoundsVar::from_scoped_type(&**ty, &scope)?;
                Ok((type_, trait_bounds))
            })
            .try_collect()?;

        // insert trait bounds
        for (predicate, bounds) in fn_args {
            scope.insert_trait_bounds(predicate, bounds);
        }
    }

    // trait bound of output type
    let output_trait_bounds = match output {
        ReturnType::Default => TraitBoundsVar::new(),
        ReturnType::Type(_, ty) => TraitBoundsVar::from_scoped_type(ty, &scope)?,
    };

    let env = {
        FnEnv {
            scope: Scope::new(),
        }
    };

    translate_block(&block, env)?;

    // quote! { #(#impls)* }
    todo!();
}

fn translate_block(block: &Block, env: FnEnv) -> syn::Result<()> {
    for stmt in block.stmts.iter() {
        match stmt {
            Stmt::Local(local) => {
                let (pat, expr) = match local {
                    Local {
                        pat,
                        init: Some((_eq, expr)),
                        ..
                    } => (pat, expr),
                    _ => return Err(Error::new(local.span(), "no initial type")),
                };

                let (local_ident, local_bounds) = match pat {
                    Pat::Ident(pat_ident) => (&pat_ident.ident, vec![]),
                    Pat::Type(PatType { pat, ty, .. }) => {
                        let ident = match &**pat {
                            Pat::Ident(pat_ident) => &pat_ident.ident,
                            _ => return Err(Error::new(pat.span(), "not an identifier")),
                        };
                        (ident, ty_to_trait_bounds(&*ty)?)
                    }
                    _ => return Err(Error::new(pat.span(), "not a valid binding")),
                };
            }
            Stmt::Item(item) => {
                return Err(Error::new(item.span(), "in-block item is not supported"))
            }
            Stmt::Expr(expr) => {
                todo!();
            }
            Stmt::Semi(expr, _semi) => {
                todo!();
            }
        }
    }

    todo!();
}

fn translate_expr(expr: &Expr, env: FnEnv) -> Vec<FnOutput> {
    match expr {
        Expr::Match(match_) => translate_match_expr(match_, env),
        Expr::Path(path) => translate_path_expr(path, env),
        Expr::Tuple(tuple) => translate_tuple_expr(tuple, env),
        Expr::Binary(binop) => translate_binary_expr(binop, env),
        Expr::If(if_) => translate_if_expr(if_, env),
        Expr::Block(block) => translate_block_expr(block, env),
        Expr::Call(call) => translate_call_expr(call, env),
        Expr::Paren(paren) => translate_expr(&paren.expr, env),
        // Expr::Let(let_) => translate_let_expr(let_, env),
        _ => todo!("expr: {:?}", expr),
    }
}

fn translate_match_expr(match_: &ExprMatch, env: FnEnv) -> Vec<FnOutput> {
    // let matched_ident = if let Expr::Path(path) = &*match_.expr {
    //     path.path.get_ident().unwrap()
    // } else {
    //     todo!("match expr")
    // };

    // let match_ty = ident_to_type(matched_ident);
    // let arg_idx = env
    //     .args
    //     .iter()
    //     .position(|arg| arg == &match_ty)
    //     .expect(&format!("Invalid match on {}", matched_ident));

    // match_
    //     .arms
    //     .iter()
    //     .map(|arm| {
    //         let (variant, fields) = translate_pat(&arm.pat);

    //         let mut env = env.clone();
    //         let field_names: Vec<_> = fields.iter().cloned().map(|(ident, _)| ident).collect();

    //         // if args = [X, Y] and expr is match Y { Q(Z) => ... }
    //         // then update args to be [X, Q<Z>]
    //         let new_type: Type = syn::parse2(quote! {
    //           #variant<#(#field_names),*>
    //         })
    //         .unwrap();
    //         env.args[arg_idx] = new_type.clone();

    //         // if quantifiers = [X, Y] then replace [Y] with [Z]
    //         env.quantifiers = env
    //             .quantifiers
    //             .into_iter()
    //             .filter(|ident| ident != matched_ident)
    //             .collect();
    //         env.quantifiers.extend(field_names);

    //         // if bounds = {Y: Foo<X>} then rename to {Q<Z>: Foo<X>}
    //         let bounds = env.bounds.remove(&ident_to_type(&matched_ident)).unwrap();
    //         env.bounds.insert(new_type.clone(), bounds);

    //         // add field bounds
    //         for (ident, kind) in fields.iter() {
    //             if let Some(kind) = kind_to_type(kind) {
    //                 env.bounds.insert(ident_to_type(ident), vec![kind]);
    //             }
    //         }

    //         // add substitution for Y -> Q<Z>
    //         env.substitutions.insert(matched_ident.clone(), new_type);

    //         translate_expr(&env, cur_kind, &arm.body)
    //     })
    //     .flatten()
    //     .collect::<Vec<_>>()
    todo!();
}

fn translate_path_expr(path: &ExprPath, env: FnEnv) -> Vec<FnOutput> {
    // let ident = path.path.get_ident().unwrap();
    // let ty = env
    //     .substitutions
    //     .get(&ident)
    //     .cloned()
    //     .unwrap_or_else(|| ident_to_type(ident));
    // vec![FnOutput {
    //     env: env.clone(),
    //     output_ty: quote! { #ty },
    // }]
    todo!();
}

fn translate_tuple_expr(tuple: &ExprTuple, env: FnEnv) -> Vec<FnOutput> {
    // if tuple.elems.len() == 0 {
    //     vec![FnOutput {
    //         env: env.clone(),
    //         output_ty: quote! { () },
    //     }]
    // } else {
    //     todo!("tuple")
    // }
    todo!();
}

fn translate_binary_expr(binop: &ExprBinary, env: FnEnv) -> Vec<FnOutput> {
    // let ExprBinary {
    //     left, right, op, ..
    // } = binop;

    // let op = match op {
    //     BinOp::Eq(_) => quote! { TypeEquals },
    //     BinOp::And(_) => quote! { TAnd },
    //     BinOp::Le(_) => quote! { TLessThanEqual },
    //     BinOp::Add(_) => quote! { TAdd },
    //     BinOp::Div(_) => quote! { TDivide },
    //     BinOp::Sub(_) => quote! { TSub },
    //     _ => todo!("binop {:?}", binop.op),
    // };
    // let trans_expr: Expr = syn::parse2(quote! { #op(#left, #right) }).unwrap();
    // translate_expr(env, cur_kind, &trans_expr)
    todo!();
}

fn translate_if_expr(if_: &ExprIf, env: FnEnv) -> Vec<FnOutput> {
    // let ExprIf {
    //     cond,
    //     then_branch,
    //     else_branch,
    //     ..
    // } = if_;
    // let (_else_token, else_) = else_branch
    //     .as_ref()
    //     .expect("If expression must have an 'else'");

    // let if_name = Ident::new(&format!("TIf{}", cur_kind), Span::call_site());
    // let trans_expr: Expr = syn::parse2(quote! { #if_name(#cond, #then_branch, #else_) }).unwrap();
    // translate_expr(env, cur_kind, &trans_expr)
    todo!();
}

fn translate_block_expr(block: &ExprBlock, env: FnEnv) -> Vec<FnOutput> {
    // block.block.stmts.iter().fold(env.clone(), |env, stmt| {
    //     // TODO
    //     match stmt {
    //         Stmt::Local(local) => {}
    //         Stmt::Item(item) => {}
    //         Stmt::Expr(expr) => {}
    //         Stmt::Semi(expr, _semi) => {}
    //     };
    //     env
    // });
    todo!();
}

fn translate_call_expr(call: &ExprCall, env: FnEnv) -> Vec<FnOutput> {
    // let ExprCall { func, args, .. } = call;

    // let func_ident = if let Expr::Path(path) = &**func {
    //     path.path.get_ident().unwrap()
    // } else {
    //     todo!("func ident")
    // };

    // let args_vec: Vec<_> = args
    //     .iter()
    //     .map(|arg| translate_expr(env, cur_kind, arg))
    //     .collect();

    // FnOutput::merge(args_vec, |mut env, args| {
    //     let first_arg: Type = syn::parse2(args[0].clone()).unwrap();
    //     let bounds = env
    //         .bounds
    //         .entry(first_arg.clone())
    //         .or_insert_with(|| Vec::new());
    //     let compute_ident = Ident::new(&format!("Compute{}", func_ident), Span::call_site());
    //     let remaining_args = &args[1..];
    //     bounds.push(syn::parse2(quote! { #compute_ident<#(#remaining_args),*> }).unwrap());

    //     let output_ty = quote! { #func_ident<#(#args),*> };
    //     FnOutput { output_ty, env }
    // })
    todo!();
}

fn translate_pat(pat: &Pat) -> (Ident, Vec<(Ident, Ident)>) {
    // match pat {
    //     Pat::Ident(PatIdent { ident, .. }) => (ident.clone(), vec![]),
    //     Pat::TupleStruct(tuple_struct) => {
    //         let variant = tuple_struct.path.get_ident().unwrap().clone();
    //         let fields = tuple_struct
    //             .pat
    //             .elems
    //             .iter()
    //             .map(|p| {
    //                 if let Pat::Ident(PatIdent { ident, subpat, .. }) = &p {
    //                     let kind = if let Some((_, p2)) = subpat {
    //                         if let Pat::Ident(PatIdent { ident, .. }) = &**p2 {
    //                             ident.clone()
    //                         } else {
    //                             panic!("RHS of @ must be an ident")
    //                         }
    //                     } else {
    //                         panic!("Match tuple struct must have @ trait annotation")
    //                     };

    //                     (ident.clone(), kind)
    //                 } else {
    //                     todo!("match pat ident")
    //                 }
    //             })
    //             .collect::<Vec<_>>();
    //         (variant, fields)
    //     }
    //     _ => todo!("match pat"),
    // }
    todo!();
}

fn ty_to_trait_bounds(ty: &Type) -> syn::Result<Vec<&Path>> {
    let bounds = match ty {
        Type::Infer(_) => vec![],
        Type::Path(path) => vec![&path.path],
        Type::TraitObject(tobj) => {
            let paths = tobj
                .bounds
                .iter()
                .map(|param_bound| match param_bound {
                    TypeParamBound::Trait(bound) => Ok(&bound.path),
                    TypeParamBound::Lifetime(lifetime) => {
                        Err(Error::new(lifetime.span(), "lifetime is not allowed"))
                    }
                })
                .try_collect()?;
            paths
        }
        _ => {
            return Err(Error::new(ty.span(), "not a trait bound"));
        }
    };

    Ok(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
}

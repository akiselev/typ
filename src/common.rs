pub use by_address::ByAddress;
pub use indexmap::{IndexMap, IndexSet};
pub use itertools::Itertools;
pub use proc_macro2::{Span, TokenStream};
pub use quote::{format_ident, quote, quote_spanned, ToTokens};
pub use radix_trie::{Trie, TrieKey};
pub use std::{
    borrow::{Borrow, BorrowMut, Cow},
    cell::{Cell, Ref, RefCell, RefMut},
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    convert::{TryFrom, TryInto},
    fmt::Debug,
    hash::Hash,
    hash::Hasher,
    iter,
    iter::{Extend, FromIterator, Sum},
    mem,
    ops::{Add, AddAssign, Deref, DerefMut},
    rc::{Rc, Weak},
};
pub use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token,
    visit_mut::VisitMut,
    Arm, AttrStyle, Attribute, BinOp, Block, Error, Expr, ExprAssign, ExprBinary, ExprBlock,
    ExprCall, ExprIf, ExprLet, ExprLit, ExprMatch, ExprPath, ExprTuple, ExprUnary, Field, Fields,
    FnArg, GenericArgument, GenericParam, Ident, ImplItem, ImplItemMethod, ImplItemType, Item,
    ItemEnum, ItemFn, ItemImpl, ItemStruct, Lit, LitBool, LitInt, Local, Pat, PatIdent, PatPath,
    PatType, Path, PathArguments, PathSegment, PredicateType, QSelf, Receiver, ReturnType,
    Signature, Stmt, TraitBound, TraitBoundModifier, Type, TypeParam, TypeParamBound, TypeParen,
    TypePath, TypeTuple, UnOp, Variant, Visibility, WherePredicate,
};

pub const IDENT_PREFIX: &str = "__TYP__";

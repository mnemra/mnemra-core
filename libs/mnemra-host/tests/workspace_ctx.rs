//! RED-phase structural invariant tests for `WorkspaceCtx` and `Role`.
//!
//! # Purpose
//!
//! Task 13 is responsible for creating:
//!   - `libs/mnemra-host/auth/workspace_ctx.rs` — the canonical `WorkspaceCtx` struct
//!   - `libs/mnemra-host/auth/role.rs`           — the `Role` enum
//!
//! These tests parse those source files with `syn` and assert their structural
//! invariants. Until Task 13 creates the files, every test here fails red for
//! the right reason: the source file is absent.
//!
//! Additionally, these tests assert that `abi/host_fns.rs` *imports*
//! `WorkspaceCtx` from the canonical `auth` module rather than defining it
//! locally. Currently, `host_fns.rs` contains a transitional stub
//! (`pub struct WorkspaceCtx { pub workspace_id: String }`) — the import
//! assertion fails red until Task 13 reconciles the duplicate.
//!
//! # RED-phase invariant
//!
//! verify: [] by design — these tests are expected to fail until Task 13 lands.
//! A passing test in this file before Task 13 is a defect (it would assert
//! nothing useful). The failure reason must be "source file absent" or
//! "wrong shape" — never a compile error or a Rust panic.
//!
//! # Spec requirements traced
//!
//! - R-0006-a: every host-fn takes `WorkspaceCtx` as its first typed param;
//!   `abi/host_fns.rs` must *import* WorkspaceCtx from `auth`, not define it
//! - R-0006-b: `WorkspaceCtx` constructed at single site after token validation;
//!   no alternative production construction path
//! - R-0006-c: `workspace_id` field is private (`workspace_id: Uuid`), public
//!   accessor `pub fn workspace_id(&self) -> Uuid`
//! - R-0006-f: test-only constructor gated with `#[cfg(test)]`
//! - R-0009-a: `Role` is a binary enum: `Admin` and `ReadObserver`
//! - R-0009-b: `WorkspaceCtx` carries `workspace_id: Uuid`, `role: Role`,
//!   `token_id: Uuid`
//!
//! # Seam to Task 13
//!
//! Task 13 implements to the following contract (inferred from these tests):
//!
//! File: `libs/mnemra-host/auth/workspace_ctx.rs`
//!
//! ```rust
//! // (NOT the actual file — the contract these tests pin)
//! use uuid::Uuid;
//! use crate::auth::role::Role;
//!
//! pub struct WorkspaceCtx {
//!     workspace_id: Uuid,   // private field
//!     pub role: Role,
//!     pub token_id: Uuid,
//! }
//!
//! impl WorkspaceCtx {
//!     /// Production constructor — called once, after token validation.
//!     pub fn new(workspace_id: Uuid, role: Role, token_id: Uuid) -> Self {
//!         Self { workspace_id, role, token_id }
//!     }
//!
//!     /// Public accessor for the private `workspace_id` field (R-0006-c).
//!     pub fn workspace_id(&self) -> Uuid {
//!         self.workspace_id
//!     }
//!
//!     /// Test-only constructor, `#[cfg(test)]`-gated (R-0006-f).
//!     #[cfg(test)]
//!     pub fn for_test(workspace_id: Uuid, role: Role, token_id: Uuid) -> Self {
//!         Self { workspace_id, role, token_id }
//!     }
//! }
//! ```
//!
//! File: `libs/mnemra-host/auth/role.rs`
//!
//! ```rust
//! pub enum Role {
//!     Admin,
//!     ReadObserver,
//! }
//! ```
//!
//! In `libs/mnemra-host/abi/host_fns.rs`, the local `WorkspaceCtx` stub MUST be
//! removed and replaced with:
//!
//! ```rust
//! use crate::auth::workspace_ctx::WorkspaceCtx;
//! ```

use std::path::Path;
use syn::{Fields, FnArg, ImplItem, Item, Visibility};

// ---------------------------------------------------------------------------
// Helpers: path resolution
// ---------------------------------------------------------------------------

/// Absolute path to `libs/mnemra-host/` inside the workspace.
fn host_lib_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR")))
}

/// Absolute path to `libs/mnemra-host/auth/workspace_ctx.rs`.
fn workspace_ctx_src() -> std::path::PathBuf {
    host_lib_dir().join("auth/workspace_ctx.rs")
}

/// Absolute path to `libs/mnemra-host/auth/role.rs`.
fn role_src() -> std::path::PathBuf {
    host_lib_dir().join("auth/role.rs")
}

/// Absolute path to `libs/mnemra-host/abi/host_fns.rs`.
fn host_fns_src() -> std::path::PathBuf {
    host_lib_dir().join("abi/host_fns.rs")
}

/// Read and parse a Rust source file with `syn`.
/// Panics with a descriptive message if the file is absent or unparseable.
fn parse_file(path: &std::path::Path) -> syn::File {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {} — Task 13 must create this file",
            path.display(),
            e
        )
    });
    syn::parse_file(&src).unwrap_or_else(|e| panic!("cannot parse {}: {}", path.display(), e))
}

// ---------------------------------------------------------------------------
// R-0009-a: Role is exactly Admin + ReadObserver (binary enum)
// ---------------------------------------------------------------------------

/// R-0009-a: `Role` at `auth/role.rs` is a binary enum with exactly
/// `Admin` and `ReadObserver` variants (no other roles).
///
/// RED: `auth/role.rs` does not exist until Task 13.
#[test]
fn role_is_binary_enum_admin_and_read_observer() {
    // R-0009-a
    let ast = parse_file(&role_src());

    let mut found_enum = false;
    let mut variants: Vec<String> = Vec::new();

    for item in &ast.items {
        if let Item::Enum(e) = item
            && e.ident == "Role"
        {
            found_enum = true;
            for v in &e.variants {
                variants.push(v.ident.to_string());
            }
        }
    }

    assert!(
        found_enum,
        "auth/role.rs must define a `Role` enum (R-0009-a) — Task 13 must create this"
    );
    assert_eq!(
        variants.len(),
        2,
        "Role enum must have exactly 2 variants (R-0009-a); got: {:?}",
        variants
    );
    assert!(
        variants.contains(&"Admin".to_string()),
        "Role must have an `Admin` variant (R-0009-a); got: {:?}",
        variants
    );
    assert!(
        variants.contains(&"ReadObserver".to_string()),
        "Role must have a `ReadObserver` variant (R-0009-a); got: {:?}",
        variants
    );
}

// ---------------------------------------------------------------------------
// R-0009-b: WorkspaceCtx struct carries workspace_id, role, token_id
// ---------------------------------------------------------------------------

/// R-0009-b: `WorkspaceCtx` at `auth/workspace_ctx.rs` carries
/// `workspace_id: Uuid`, `role: Role`, `token_id: Uuid` as fields.
///
/// RED: `auth/workspace_ctx.rs` does not exist until Task 13.
#[test]
fn workspace_ctx_carries_required_fields() {
    // R-0009-b
    let ast = parse_file(&workspace_ctx_src());

    let mut found_struct = false;
    let mut field_names: Vec<String> = Vec::new();
    let mut field_types: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for item in &ast.items {
        if let Item::Struct(s) = item
            && s.ident == "WorkspaceCtx"
        {
            found_struct = true;
            if let Fields::Named(named) = &s.fields {
                for field in &named.named {
                    if let Some(ident) = &field.ident {
                        let name = ident.to_string();
                        let ty = quote_type(&field.ty);
                        field_types.insert(name.clone(), ty);
                        field_names.push(name);
                    }
                }
            }
        }
    }

    assert!(
        found_struct,
        "auth/workspace_ctx.rs must define a `WorkspaceCtx` struct (R-0009-b) — Task 13 must create this"
    );

    // workspace_id, role, token_id must all be present
    for required in &["workspace_id", "role", "token_id"] {
        assert!(
            field_names.contains(&required.to_string()),
            "WorkspaceCtx must have field `{}` (R-0009-b); fields found: {:?}",
            required,
            field_names
        );
    }

    // workspace_id must use Uuid
    if let Some(ty) = field_types.get("workspace_id") {
        assert!(
            ty.contains("Uuid"),
            "WorkspaceCtx.workspace_id must be typed Uuid (R-0009-b, R-0006-c); got: {}",
            ty
        );
    }

    // role must use Role
    if let Some(ty) = field_types.get("role") {
        assert!(
            ty.contains("Role"),
            "WorkspaceCtx.role must be typed Role (R-0009-b, R-0009-a); got: {}",
            ty
        );
    }

    // token_id must use Uuid
    if let Some(ty) = field_types.get("token_id") {
        assert!(
            ty.contains("Uuid"),
            "WorkspaceCtx.token_id must be typed Uuid (R-0009-b); got: {}",
            ty
        );
    }
}

// ---------------------------------------------------------------------------
// R-0006-c: workspace_id is a PRIVATE field
// ---------------------------------------------------------------------------

/// R-0006-c: `WorkspaceCtx.workspace_id` must be a private field (no `pub`
/// visibility). Direct field access from outside the module is not allowed.
///
/// RED: `auth/workspace_ctx.rs` does not exist until Task 13.
/// ALSO RED on the current stub: host_fns.rs has `pub workspace_id: String`.
#[test]
fn workspace_ctx_workspace_id_field_is_private() {
    // R-0006-c
    let ast = parse_file(&workspace_ctx_src());

    let mut found_struct = false;
    let mut workspace_id_visibility: Option<String> = None;

    for item in &ast.items {
        if let Item::Struct(s) = item
            && s.ident == "WorkspaceCtx"
        {
            found_struct = true;
            if let Fields::Named(named) = &s.fields {
                for field in &named.named {
                    if field
                        .ident
                        .as_ref()
                        .map(|i| i == "workspace_id")
                        .unwrap_or(false)
                    {
                        workspace_id_visibility = Some(format!("{:?}", field.vis));
                    }
                }
            }
        }
    }

    assert!(
        found_struct,
        "auth/workspace_ctx.rs must define a `WorkspaceCtx` struct (R-0006-c) — Task 13 must create this"
    );

    let vis =
        workspace_id_visibility.expect("WorkspaceCtx must have a `workspace_id` field (R-0006-c)");
    // Inherited visibility (no `pub`) means the field is private.
    assert!(
        vis.contains("Inherited"),
        "WorkspaceCtx.workspace_id must be a private field (R-0006-c) — no `pub` visibility; got: {}",
        vis
    );
}

// ---------------------------------------------------------------------------
// R-0006-c: public accessor `workspace_id(&self) -> Uuid` exists
// ---------------------------------------------------------------------------

/// R-0006-c: `WorkspaceCtx` must expose a public `workspace_id(&self) -> Uuid`
/// accessor method. The field is private; access is only via this accessor.
///
/// RED: `auth/workspace_ctx.rs` does not exist until Task 13.
#[test]
fn workspace_ctx_has_public_workspace_id_accessor() {
    // R-0006-c
    let ast = parse_file(&workspace_ctx_src());

    let mut found_accessor = false;
    let mut accessor_is_pub = false;
    let mut accessor_return_type = String::new();

    for item in &ast.items {
        if let Item::Impl(impl_block) = item {
            // Look for impl WorkspaceCtx (no trait)
            if impl_block.trait_.is_none() && type_name_is(&impl_block.self_ty, "WorkspaceCtx") {
                for impl_item in &impl_block.items {
                    if let ImplItem::Fn(method) = impl_item
                        && method.sig.ident == "workspace_id"
                    {
                        found_accessor = true;
                        accessor_is_pub = matches!(method.vis, Visibility::Public(_));
                        accessor_return_type = quote_return_type(&method.sig.output);

                        // Verify it takes &self (not self or &mut self)
                        let first_param = method.sig.inputs.first();
                        let takes_ref_self = first_param.map(|p| {
                                matches!(p, FnArg::Receiver(r) if r.reference.is_some() && r.mutability.is_none())
                            }).unwrap_or(false);
                        assert!(
                            takes_ref_self,
                            "WorkspaceCtx::workspace_id must take `&self` (R-0006-c)"
                        );
                    }
                }
            }
        }
    }

    assert!(
        found_accessor,
        "WorkspaceCtx must have a `workspace_id` accessor method (R-0006-c) — Task 13 must add this"
    );
    assert!(
        accessor_is_pub,
        "WorkspaceCtx::workspace_id accessor must be `pub` (R-0006-c)"
    );
    assert!(
        accessor_return_type.contains("Uuid"),
        "WorkspaceCtx::workspace_id must return Uuid (R-0006-c); got return type: {}",
        accessor_return_type
    );
}

// ---------------------------------------------------------------------------
// R-0006-f: test-only constructor is #[cfg(test)]-gated
// ---------------------------------------------------------------------------

/// R-0006-f: `WorkspaceCtx` must expose a test-only constructor that is
/// gated with `#[cfg(test)]`. This makes it impossible to call in production
/// code (the compiler won't compile it outside `#[cfg(test)]` contexts).
///
/// RED: `auth/workspace_ctx.rs` does not exist until Task 13.
#[test]
fn workspace_ctx_test_constructor_is_cfg_test_gated() {
    // R-0006-f
    let ast = parse_file(&workspace_ctx_src());

    let mut found_test_ctor = false;
    let mut has_cfg_test = false;

    for item in &ast.items {
        if let Item::Impl(impl_block) = item
            && impl_block.trait_.is_none()
            && type_name_is(&impl_block.self_ty, "WorkspaceCtx")
        {
            for impl_item in &impl_block.items {
                if let ImplItem::Fn(method) = impl_item {
                    // Look for a method that is either named `for_test` or `test_*`
                    // or carries `#[cfg(test)]`
                    let method_name = method.sig.ident.to_string();
                    let attrs_have_cfg_test = method.attrs.iter().any(|attr| {
                        let attr_str = format!("{}", quote::quote!(#attr));
                        attr_str.contains("cfg") && attr_str.contains("test")
                    });

                    // Accept: method named `for_test` or any test-constructor pattern
                    let looks_like_test_ctor = method_name.contains("test")
                        || method_name == "for_test"
                        || attrs_have_cfg_test;

                    if looks_like_test_ctor || attrs_have_cfg_test {
                        // Also verify it's not a `&self` method (it's a constructor)
                        let is_associated_fn = !method
                            .sig
                            .inputs
                            .iter()
                            .any(|a| matches!(a, FnArg::Receiver(_)));
                        if is_associated_fn || attrs_have_cfg_test {
                            found_test_ctor = true;
                            has_cfg_test = attrs_have_cfg_test;
                        }
                    }
                }
            }
        }
    }

    assert!(
        found_test_ctor,
        "WorkspaceCtx must have a test-only constructor method (R-0006-f) — Task 13 must add this (e.g., `for_test`)"
    );
    assert!(
        has_cfg_test,
        "WorkspaceCtx test constructor must be gated with `#[cfg(test)]` (R-0006-f) — found a candidate but missing the gate"
    );
}

// ---------------------------------------------------------------------------
// R-0006-b: single production construction site (whole-tree scan)
// ---------------------------------------------------------------------------

/// R-0006-b: `WorkspaceCtx` must be constructed at exactly ONE location in the
/// whole crate (after token validation in the auth resolve path). This test
/// performs a **whole-tree scan** of `libs/mnemra-host/` — excluding the
/// `tests/` directory and `#[cfg(test)]`-gated items — to count production call
/// sites for `WorkspaceCtx::new(` and `WorkspaceCtx { .. }` struct literals.
///
/// The scan is directory-walk-based (not a hardcoded file list) so it keeps
/// guarding as future tasks add new source files. A hardcoded file list silently
/// stops guarding when construction moves to a new file.
///
/// Detection strategy (resilient to quote! spacing):
/// - Parse function BODIES only (not signatures/return types) to avoid false
///   positives from `-> WorkspaceCtx` or `fn new(…) -> WorkspaceCtx`.
/// - Normalize whitespace in the body token stream before matching, which makes
///   detection insensitive to how `quote!` renders `::`-spacing.
/// - The `fn new` definition body constructs via `Self { .. }`, never
///   `WorkspaceCtx::new(` or `WorkspaceCtx {`, so it is excluded naturally.
/// - `for_test` is `#[cfg(test)]`-gated and excluded by the cfg-test filter.
///
/// RED: Zero construction sites until Task 13 adds `auth/resolve.rs`.
/// GREEN: Exactly 1 site — `auth/resolve.rs::from_token` (line 34).
/// FUTURE: Any second production call will be caught immediately, regardless
///         of which file it appears in.
#[test]
fn workspace_ctx_has_exactly_one_production_construction_site() {
    // R-0006-b
    let host_dir = host_lib_dir();
    let mut production_construction_sites: Vec<String> = Vec::new();

    collect_production_construction_sites(host_dir, &mut production_construction_sites);

    assert_eq!(
        production_construction_sites.len(),
        1,
        "WorkspaceCtx must be constructed at exactly ONE production site (R-0006-b). \
        Found {} site(s) across whole crate tree: {:?}. \
        If count is 0 — Task 13 has not yet wired the construction site. \
        If count > 1 — a second construction site has been added outside the \
        auth/resolve seam; that violates the tenant-isolation invariant.",
        production_construction_sites.len(),
        production_construction_sites
    );
}

/// Recursively walk `root` for `*.rs` files, skipping the `tests/` subdirectory.
/// For each file, parse with syn and scan non-test function bodies for
/// `WorkspaceCtx::new(` or `WorkspaceCtx{` call sites.
/// Matching site labels (file + function name) are appended to `sites`.
fn collect_production_construction_sites(root: &std::path::Path, sites: &mut Vec<String>) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip the tests/ directory entirely — those are test files.
            if path.file_name().map(|n| n == "tests").unwrap_or(false) {
                continue;
            }
            collect_production_construction_sites(&path, sites);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            let src = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let ast = match syn::parse_file(&src) {
                Ok(a) => a,
                Err(_) => continue,
            };
            scan_file_for_construction_call_sites(&ast, &path, sites);
        }
    }
}

/// Scan all non-test function bodies in `ast` for `WorkspaceCtx` construction
/// call expressions.
///
/// Detection uses normalized body text (whitespace-collapsed token stream of the
/// function block only) to avoid false positives from signatures/return types and
/// to remain insensitive to `quote!` rendering of `::` spacing.
///
/// The marker strings after normalization are:
///   - `"WorkspaceCtx::new("` — call of the production constructor
///   - `"WorkspaceCtx{"` — struct literal (blocked by private field outside the
///     defining module, but scanned for defense in depth)
fn scan_file_for_construction_call_sites(
    ast: &syn::File,
    path: &std::path::Path,
    sites: &mut Vec<String>,
) {
    scan_items_for_construction_call_sites(&ast.items, path, sites);
}

/// Recursively scan a slice of items, honoring cfg(test) gates at every level.
fn scan_items_for_construction_call_sites(
    items: &[Item],
    path: &std::path::Path,
    sites: &mut Vec<String>,
) {
    for item in items {
        if item_has_cfg_test(item) {
            continue;
        }
        match item {
            Item::Fn(func) => {
                if func_has_cfg_test(func) {
                    continue;
                }
                check_fn_body_for_construction(
                    &func.block,
                    &func.sig.ident.to_string(),
                    path,
                    sites,
                );
            }
            Item::Impl(impl_block) => {
                if attrs_have_cfg_test(&impl_block.attrs) {
                    continue;
                }
                for impl_item in &impl_block.items {
                    if let ImplItem::Fn(method) = impl_item {
                        if method_has_cfg_test(method) {
                            continue;
                        }
                        let fn_name = method.sig.ident.to_string();
                        // Skip test-gated name patterns (belt-and-suspenders).
                        if fn_name == "for_test" || fn_name.starts_with("test_") {
                            continue;
                        }
                        check_fn_body_for_construction(&method.block, &fn_name, path, sites);
                    }
                }
            }
            Item::Mod(m) => {
                if attrs_have_cfg_test(&m.attrs) {
                    continue;
                }
                // Recurse into inline module bodies.
                if let Some((_, mod_items)) = &m.content {
                    scan_items_for_construction_call_sites(mod_items, path, sites);
                }
            }
            _ => {}
        }
    }
}

/// Render a function block as a whitespace-normalized token string, then check
/// for `WorkspaceCtx::new(` or `WorkspaceCtx{` in the body.
/// If found, record `"<file_relative>::<fn_name>"` in `sites`.
fn check_fn_body_for_construction(
    block: &syn::Block,
    fn_name: &str,
    path: &std::path::Path,
    sites: &mut Vec<String>,
) {
    // Render body tokens and normalize whitespace.
    // This avoids `-> WorkspaceCtx` in the signature triggering a false positive,
    // and sidesteps any quote! rendering quirks around `::` spacing.
    let body_tokens = format!("{}", quote::quote!(#block));
    let body_normalized: String = body_tokens.split_whitespace().collect();

    let has_new_call = body_normalized.contains("WorkspaceCtx::new(");
    let has_struct_literal = body_normalized.contains("WorkspaceCtx{");

    if has_new_call || has_struct_literal {
        // Compute a human-readable relative path label for the failure message.
        let label = format!("{}::{}", path.display(), fn_name);
        sites.push(label);
    }
}

// ---------------------------------------------------------------------------
// R-0006-a (canonical type): host_fns.rs imports WorkspaceCtx from auth
// ---------------------------------------------------------------------------

/// R-0006-a (canonical type): `abi/host_fns.rs` must NOT define `WorkspaceCtx`
/// locally. It must import it from `crate::auth::workspace_ctx` (or re-export
/// via `crate::auth`). This ensures there is a single canonical type, not a
/// stub and a real type coexisting.
///
/// RED: Currently, `host_fns.rs` defines `pub struct WorkspaceCtx { pub workspace_id: String }`.
/// This test fails until Task 13 removes that definition and adds the import.
#[test]
fn host_fns_imports_workspace_ctx_from_auth_not_defines_locally() {
    // R-0006-a (canonical type enforcement)
    let ast = parse_file(&host_fns_src());

    let mut defines_workspace_ctx_locally = false;

    for item in &ast.items {
        if let Item::Struct(s) = item
            && s.ident == "WorkspaceCtx"
        {
            defines_workspace_ctx_locally = true;
        }
    }

    assert!(
        !defines_workspace_ctx_locally,
        "abi/host_fns.rs must NOT define WorkspaceCtx locally (R-0006-a). \
        Found a local `pub struct WorkspaceCtx` definition — Task 13 must remove this stub \
        and import `WorkspaceCtx` from `crate::auth::workspace_ctx`. \
        The local stub has the wrong shape (`pub workspace_id: String`) and \
        must not coexist with the canonical auth type."
    );
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Format a `syn::Type` as a string for display/assertion messages.
fn quote_type(ty: &syn::Type) -> String {
    format!("{}", quote::quote!(#ty))
}

/// Format a `syn::ReturnType` as a string.
fn quote_return_type(ret: &syn::ReturnType) -> String {
    format!("{}", quote::quote!(#ret))
}

/// Return true if a `syn::Type` is a path type whose final segment matches `name`.
fn type_name_is(ty: &syn::Type, name: &str) -> bool {
    if let syn::Type::Path(tp) = ty {
        tp.path
            .segments
            .last()
            .map(|s| s.ident == name)
            .unwrap_or(false)
    } else {
        false
    }
}

/// Return true if any attribute on this item is `#[cfg(test)]`.
fn item_has_cfg_test(item: &Item) -> bool {
    let attrs = match item {
        Item::Fn(f) => &f.attrs,
        Item::Mod(m) => &m.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Struct(s) => &s.attrs,
        Item::Enum(e) => &e.attrs,
        _ => return false,
    };
    attrs_have_cfg_test(attrs)
}

fn method_has_cfg_test(method: &syn::ImplItemFn) -> bool {
    attrs_have_cfg_test(&method.attrs)
}

fn func_has_cfg_test(func: &syn::ItemFn) -> bool {
    attrs_have_cfg_test(&func.attrs)
}

fn attrs_have_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let s = format!("{}", quote::quote!(#attr));
        s.contains("cfg") && s.contains("test")
    })
}

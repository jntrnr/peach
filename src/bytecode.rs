use std::collections::HashMap;

use syn::{self, Item, ItemFn, ItemMod};

use typecheck::Ty;

type VarId = usize;
type Offset = usize;

#[derive(Debug, Clone)]
pub enum Bytecode {
    ReturnLastStackValue,
    ReturnVoid,
    PushU64(u64),
    PushU32(u32),
    PushBool(bool),
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    VarDecl(VarId),
    VarDeclUninit(VarId),
    Var(VarId),
    Assign(VarId),
    Call(DefinitionId),
    If(Offset, Ty), // Offset is number of bytecodes to jump forward if false.  Also includes the type of the result, if this is an expression
    Else(Offset, Ty), // Offset is number of bytecodes to skip (aka jump forward). Also includes the type of the result, if this is an expression
    EndIf(Ty),        //includes the type of the result, if this is an expression
    BeginWhile,
    WhileCond(Offset), // Offset is number of bytecodes to jump forward if false
    EndWhile(Offset),  // Offset is number of bytecodes to jump backward to return to start of while
    DebugPrint,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub(crate) var_id: VarId,
    pub ty: Ty,
}
impl Param {
    pub fn new(name: String, var_id: VarId, ty: Ty) -> Param {
        Param { name, var_id, ty }
    }
}

//TODO: should VarDecl and Param be merged?
#[derive(Clone, Debug)]
pub struct VarDecl {
    pub ident: String,
    pub ty: Ty,
}

impl VarDecl {
    fn new(ident: String, ty: Ty) -> VarDecl {
        VarDecl { ident, ty }
    }
}

#[derive(Clone)]
pub struct VarStack {
    var_stack: Vec<usize>,
    pub(crate) vars: Vec<VarDecl>,
}

impl VarStack {
    pub fn new() -> VarStack {
        VarStack {
            var_stack: vec![],
            vars: vec![],
        }
    }

    pub(crate) fn add_var(&mut self, ident: String, ty: Ty) -> usize {
        self.vars.push(VarDecl::new(ident, ty));
        let pos = self.vars.len() - 1;
        self.var_stack.push(pos);
        pos
    }

    //TODO: this probably should be a Result in the future
    pub fn find_var(&self, ident: &str) -> Option<usize> {
        for var in self.var_stack.iter().rev() {
            if ident == &self.vars[*var].ident {
                return Some(*var);
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct Fun {
    pub params: Vec<Param>,
    pub return_ty: Ty,
    pub vars: Vec<VarDecl>,
    pub bytecode: Vec<Bytecode>,
}

#[derive(Debug, Clone)]
pub struct Mod {
    pub scope_id: ScopeId,
}
impl Mod {
    pub fn new(scope_id: ScopeId) -> Mod {
        Mod { scope_id }
    }
}

pub(crate) type ScopeId = usize;
pub(crate) type DefinitionId = usize;

#[derive(Clone, Debug)]
pub(crate) enum Lazy {
    ItemFn(ItemFn),
    ItemMod(ItemMod),
}

#[derive(Clone, Debug)]
pub(crate) enum Processed {
    Fun(Fun),
    Mod(Mod),
}

#[derive(Clone, Debug)]
pub(crate) enum DefinitionState {
    Lazy(Lazy),
    Processed(Processed),
}

pub struct Scope {
    parent: Option<ScopeId>,
    is_mod: bool,
    pub(crate) definitions: HashMap<String, DefinitionId>,
}

impl Scope {
    pub(crate) fn new(parent: Option<ScopeId>, is_mod: bool) -> Scope {
        Scope {
            parent,
            is_mod,
            definitions: HashMap::new(),
        }
    }
}

/// BytecodeEngine is the root of Peach's work.  Here code is converted from source files to an intermediate bytecode format
/// First, the file is parsed into an AST.  Once an AST, further computation is delayed until definitions are required.
/// This allows conversion from AST to definitions to happen lazily.
///
/// No processing is done by default.  Once a file is loaded, you must then process the file by giving a function name to begin with.
/// Eg)
/// ```no_run
/// extern crate peachlib;
/// use peachlib::BytecodeEngine;
///
/// let mut bc = BytecodeEngine::new();
/// bc.load_file("bin.rs");
/// bc.process_fn("main", 0);
/// ```
/// Processing is done on function granularity.  As definitions are referenced in the function, they too are processed.
pub struct BytecodeEngine {
    pub(crate) scopes: Vec<Scope>,
    pub(crate) definitions: Vec<DefinitionState>,
    pub(crate) project_root: Option<::std::path::PathBuf>,
}

impl BytecodeEngine {
    pub fn new() -> BytecodeEngine {
        BytecodeEngine {
            scopes: vec![
                Scope {
                    parent: None,
                    is_mod: true,
                    definitions: HashMap::new(),
                },
            ],
            definitions: vec![],
            project_root: None,
        }
    }

    /// Will find the definition id for the given name, by starting at the scope given and working up through the scopes
    /// until the matching definition is found.
    /// Returns the corresponding definition id with the scope it was found in
    fn get_defn(&self, defn_name: &str, starting_scope_id: ScopeId) -> (DefinitionId, ScopeId) {
        let mut current_scope_id = starting_scope_id;

        while !self.scopes[current_scope_id]
            .definitions
            .contains_key(defn_name)
        {
            if self.scopes[current_scope_id].is_mod {
                unimplemented!(
                    "Definition {} not found in module (or needs to be precomputed)",
                    defn_name
                );
            }
            if let Some(parent_id) = self.scopes[current_scope_id].parent {
                current_scope_id = parent_id;
            } else {
                unimplemented!("Definition {} needs to be precomputed", defn_name);
            }
        }

        (
            self.scopes[current_scope_id].definitions[defn_name],
            current_scope_id,
        )
    }

    /// Gets the bytecoded function for the given name
    pub fn get_fn(&self, defn_name: &str, scope_id: ScopeId) -> &Fun {
        let (defn_id, _) = self.get_defn(defn_name, scope_id);
        let defn = &self.definitions[defn_id];

        if let DefinitionState::Processed(Processed::Fun(ref p)) = defn {
            p
        } else {
            unimplemented!("Function {:?} needs to be precomputed", defn)
        }
    }

    /// Sets the project root that will be used when modules are loaded
    pub fn set_project_root(&mut self, path: &str) {
        use std::fs;

        let path = fs::canonicalize(path).unwrap();

        self.project_root = Some(path);
    }

    /// Loads the file with the given name
    pub fn load_file(&mut self, fname: &str) {
        use std::fs::File;
        use std::io::Read;
        let path = if let Some(ref project_path) = self.project_root {
            let mut temp_path = project_path.clone();
            temp_path.push(fname);
            temp_path
        } else {
            let mut temp_path = ::std::path::PathBuf::new();
            temp_path.push(fname);
            temp_path
        };

        let mut file = File::open(path).expect("Unable to open file");

        let mut src = String::new();
        file.read_to_string(&mut src).expect("Unable to read file");

        let syntax_file = syn::parse_file(&src).expect("Unable to parse file");

        for item in syntax_file.items {
            self.prepare_item(item, 0);
        }
    }

    /// Prepares the given item to be processed lazily
    pub fn prepare_item(&mut self, item: Item, current_scope_id: ScopeId) {
        use std::fs::File;
        use std::io::Read;

        match item {
            Item::Fn(item_fn) => {
                // Adds a function to be processed lazily
                let fn_name = item_fn.ident.to_string();
                self.definitions
                    .push(DefinitionState::Lazy(Lazy::ItemFn(item_fn)));
                self.scopes[current_scope_id]
                    .definitions
                    .insert(fn_name, self.definitions.len() - 1);
            }
            Item::Mod(item_mod) => {
                if item_mod.content.is_none() {
                    //Load the file as a module
                    let fname = item_mod.ident.as_ref();
                    let path = if let Some(ref project_path) = self.project_root {
                        let mut temp_path = project_path.clone();
                        temp_path.push(fname);
                        temp_path.set_extension("rs");
                        temp_path
                    } else {
                        let mut temp_path = ::std::path::PathBuf::new();
                        temp_path.push(fname);
                        temp_path.set_extension("rs");
                        temp_path
                    };

                    let mut file = File::open(path).expect("Unable to open file");

                    let mut src = String::new();
                    file.read_to_string(&mut src).expect("Unable to read file");

                    let syntax_file = syn::parse_file(&src).expect("Unable to parse file");
                    self.scopes.push(Scope::new(None, true));
                    let mod_scope_id = self.scopes.len() - 1;

                    // Eagerly process the top-most bit of the file as a module
                    // This allows us to make its contents lazily available
                    // Part of the reason we do it this way is that we don't have an ItemMod
                    self.definitions
                        .push(DefinitionState::Processed(Processed::Mod(Mod::new(
                            mod_scope_id,
                        ))));

                    self.scopes[current_scope_id]
                        .definitions
                        .insert(item_mod.ident.to_string(), self.definitions.len() - 1);

                    for item in syntax_file.items {
                        self.prepare_item(item, mod_scope_id);
                    }
                } else {
                    // Add module to be processed lazily
                    let mod_name = item_mod.ident.to_string();
                    self.definitions
                        .push(DefinitionState::Lazy(Lazy::ItemMod(item_mod)));
                    self.scopes[current_scope_id]
                        .definitions
                        .insert(mod_name, self.definitions.len() - 1);
                }
            }
            Item::Use(ref item_use) => {
                // Use seems to start higher up in the scopes, so start higher
                let mut temp_scope_id = current_scope_id;

                loop {
                    //TODO: FIXME: not sure if this is correct
                    if self.scopes[temp_scope_id].is_mod {
                        break;
                    }
                    if let Some(parent_id) = self.scopes[temp_scope_id].parent {
                        temp_scope_id = parent_id;
                    } else {
                        break;
                    }
                }

                self.process_use_tree(&item_use.tree, current_scope_id, temp_scope_id);
            }
            _ => {
                unimplemented!("Unknown item type: {:#?}", item);
            }
        }
    }

    /// Begin processing the lazy definitions starting at the given function.
    /// This will continue processing until all necessary definitions have been processed.
    pub fn process_fn(&mut self, fn_name: &str, scope_id: ScopeId) -> DefinitionId {
        let (definition_id, found_scope_id) = self.get_defn(fn_name, scope_id);

        let fun = self.convert_fn_to_bytecode(definition_id, found_scope_id);
        self.definitions[definition_id] = DefinitionState::Processed(Processed::Fun(fun));

        definition_id
    }

    fn process_mod(&mut self, mod_name: &str, scope_id: ScopeId) -> DefinitionId {
        let (definition_id, current_scope_id) = self.get_defn(mod_name, scope_id);

        if let DefinitionState::Lazy(Lazy::ItemMod(ref item_mod)) = self.definitions[definition_id]
        {
            self.scopes.push(Scope::new(Some(current_scope_id), true));
            let mod_scope_id = self.scopes.len() - 1;

            match item_mod.content {
                //TODO: would be great if we didn't clone here and just reused what we had
                Some(ref content) => for item in content.1.clone() {
                    self.prepare_item(item, mod_scope_id);
                },
                None => {}
            }

            self.definitions[definition_id] =
                DefinitionState::Processed(Processed::Mod(Mod::new(mod_scope_id)));
        }
        definition_id
    }

    fn process_defn(&mut self, name: &str, scope_id: ScopeId) -> DefinitionId {
        let (definition_id, scope_id) = self.get_defn(name, scope_id);

        if let DefinitionState::Lazy(ref lazy) = self.definitions[definition_id] {
            match lazy {
                Lazy::ItemFn(_) => self.process_fn(name, scope_id),
                Lazy::ItemMod(_) => self.process_mod(name, scope_id),
            }
        } else {
            definition_id
        }
    }

    /// Processes a path looking for the definition being referenced.
    /// Returns: The processed definition id of the found item
    pub(crate) fn process_path(
        &mut self,
        path: &syn::Path,
        current_scope_id: ScopeId,
    ) -> DefinitionId {
        let mut mod_scope_id = current_scope_id;
        if path.leading_colon.is_some() {
            loop {
                if let Some(parent_id) = self.scopes[mod_scope_id].parent {
                    mod_scope_id = parent_id;
                } else {
                    break;
                }
            }
        }

        let num_segments = path.segments.len();

        for current_segment in 0..(num_segments - 1) {
            let ident = path.segments[current_segment].ident.as_ref();
            let definition_id = self.process_mod(ident, mod_scope_id);
            if let DefinitionState::Processed(Processed::Mod(ref module)) =
                self.definitions[definition_id]
            {
                mod_scope_id = module.scope_id;
            } else {
                unimplemented!("Failure to process module");
            }
        }

        // from there, look in this scpoe for the name
        let num_segments = path.segments.len();
        let ident = path.segments[num_segments - 1].ident.to_string();

        // lastly, make sure we've processed the definition before we return
        self.process_defn(&ident, mod_scope_id)
    }

    fn process_use_tree(
        &mut self,
        use_tree: &syn::UseTree,
        original_scope_id: ScopeId,
        current_scope_id: ScopeId,
    ) {
        match use_tree {
            syn::UseTree::Name(ref use_name) => {
                let definition_id = self.process_defn(use_name.ident.as_ref(), current_scope_id);

                self.scopes[original_scope_id]
                    .definitions
                    .insert(use_name.ident.to_string(), definition_id);
            }
            syn::UseTree::Path(ref use_path) => {
                let definition_id = self.process_mod(use_path.ident.as_ref(), current_scope_id);
                if let DefinitionState::Processed(Processed::Mod(ref module)) =
                    self.definitions[definition_id]
                {
                    self.process_use_tree(&*use_path.tree, original_scope_id, module.scope_id);
                } else {
                    unimplemented!("Expected module in use path");
                }
            }
            syn::UseTree::Group(ref use_group) => {
                for tree in &use_group.items {
                    self.process_use_tree(tree, original_scope_id, current_scope_id);
                }
            }
            syn::UseTree::Glob(_) => {
                let mut defn_names = vec![];
                for defn_name in self.scopes[current_scope_id].definitions.keys() {
                    defn_names.push(defn_name.clone());
                }

                for defn_name in defn_names {
                    let definition_id = self.process_defn(&defn_name, current_scope_id);

                    self.scopes[original_scope_id]
                        .definitions
                        .insert(defn_name, definition_id);
                }
            }
            syn::UseTree::Rename(ref use_rename) => {
                let definition_id = self.process_defn(use_rename.ident.as_ref(), current_scope_id);

                self.scopes[original_scope_id]
                    .definitions
                    .insert(use_rename.rename.to_string(), definition_id);
            }
        }
    }

    /// immediately process a string into bytecode, treating it as an expression
    /// this is likely only useful for building REPLs
    pub fn process_raw_expr_str(
        &mut self,
        expr_str: &str,
        bytecode: &mut Vec<Bytecode>,
        var_stack: &mut VarStack,
    ) -> Result<Ty, String> {
        match syn::parse_str::<syn::Expr>(expr_str) {
            Ok(expr) => {
                Ok(self.convert_expr_to_bytecode(
                    &expr,
                    &Ty::Unknown,
                    bytecode,
                    0, // hardwire repl scope to 0
                    var_stack,
                ))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// immediately process a string into bytecode, treating it as a statement
    /// this will also process items so that their definitions are in scope
    /// this is likely only useful for building REPLs
    pub fn process_raw_stmt_str(
        &mut self,
        expr_str: &str,
        bytecode: &mut Vec<Bytecode>,
        var_stack: &mut VarStack,
    ) -> Result<(), String> {
        match syn::parse_str::<syn::Stmt>(expr_str) {
            Ok(stmt) => {
                match stmt {
                    syn::Stmt::Item(item) => {
                        self.prepare_item(item, 0);
                        Ok(())
                    }
                    _ => {
                        self.convert_stmt_to_bytecode(
                            &stmt,
                            &Ty::Unknown,
                            bytecode,
                            0, // hardwire repl scope to 0
                            var_stack,
                        );
                        Ok(())
                    }
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

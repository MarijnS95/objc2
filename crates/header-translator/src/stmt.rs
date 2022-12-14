use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;

use clang::{Entity, EntityKind, EntityVisitResult};

use crate::availability::Availability;
use crate::config::{ClassData, Config};
use crate::expr::Expr;
use crate::method::{handle_reserved, Method};
use crate::property::Property;
use crate::rust_type::{GenericType, Ty};
use crate::unexposed_macro::UnexposedMacro;

#[derive(Debug, Clone, PartialEq)]
pub enum MethodOrProperty {
    Method(Method),
    Property(Property),
}

impl fmt::Display for MethodOrProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Method(method) => write!(f, "{method}"),
            Self::Property(property) => write!(f, "{property}"),
        }
    }
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Derives(Cow<'static, str>);

impl Default for Derives {
    fn default() -> Self {
        Derives("Debug, PartialEq, Eq, Hash".into())
    }
}

impl fmt::Display for Derives {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#[derive({})]", self.0)
    }
}

/// Takes one of:
/// - `EntityKind::ObjCInterfaceDecl`
/// - `EntityKind::ObjCProtocolDecl`
/// - `EntityKind::ObjCCategoryDecl`
fn parse_objc_decl(
    entity: &Entity<'_>,
    mut superclass: Option<&mut Option<Option<GenericType>>>,
    mut generics: Option<&mut Vec<GenericType>>,
    data: Option<&ClassData>,
) -> (Vec<String>, Vec<MethodOrProperty>) {
    let mut protocols = Vec::new();
    let mut methods = Vec::new();

    // Track seen properties, so that when methods are autogenerated by the
    // compiler from them, we can skip them
    let mut properties = HashSet::new();

    entity.visit_children(|entity, _parent| {
        match entity.get_kind() {
            EntityKind::ObjCExplicitProtocolImpl if generics.is_none() && superclass.is_none() => {
                // TODO NS_PROTOCOL_REQUIRES_EXPLICIT_IMPLEMENTATION
            }
            EntityKind::ObjCIvarDecl if superclass.is_some() => {
                // Explicitly ignored
            }
            EntityKind::ObjCSuperClassRef => {
                if let Some(superclass) = &mut superclass {
                    let name = entity.get_name().expect("superclass name");
                    **superclass = Some(Some(GenericType {
                        name,
                        // These are filled out in EntityKind::TypeRef
                        generics: Vec::new(),
                    }));
                } else {
                    panic!("unsupported superclass {entity:?}");
                }
            }
            EntityKind::ObjCRootClass => {
                if let Some(superclass) = &mut superclass {
                    // TODO: Maybe just skip root classes entirely?
                    **superclass = Some(None);
                } else {
                    panic!("unsupported root class {entity:?}");
                }
            }
            EntityKind::ObjCClassRef if generics.is_some() => {
                // println!("ObjCClassRef: {:?}", entity.get_display_name());
            }
            EntityKind::TemplateTypeParameter => {
                if let Some(generics) = &mut generics {
                    // TODO: Generics with bounds (like NSMeasurement<UnitType: NSUnit *>)
                    // let ty = entity.get_type().expect("template type");
                    let name = entity.get_display_name().expect("template name");
                    generics.push(GenericType {
                        name,
                        generics: Vec::new(),
                    });
                } else {
                    panic!("unsupported generics {entity:?}");
                }
            }
            EntityKind::ObjCProtocolRef => {
                protocols.push(entity.get_name().expect("protocolref to have name"));
            }
            EntityKind::ObjCInstanceMethodDecl | EntityKind::ObjCClassMethodDecl => {
                let partial = Method::partial(entity);

                if !properties.remove(&(partial.is_class, partial.fn_name.clone())) {
                    let data = data
                        .map(|data| {
                            data.methods
                                .get(&partial.fn_name)
                                .copied()
                                .unwrap_or_default()
                        })
                        .unwrap_or_default();
                    if let Some(method) = partial.parse(data) {
                        methods.push(MethodOrProperty::Method(method));
                    }
                }
            }
            EntityKind::ObjCPropertyDecl => {
                let partial = Property::partial(entity);
                let data = data
                    .map(|data| {
                        data.properties
                            .get(&partial.name)
                            .copied()
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();

                assert!(
                    properties.insert((partial.is_class, partial.getter_name.clone())),
                    "already exisiting property"
                );
                if let Some(setter_name) = partial.setter_name.clone() {
                    assert!(
                        properties.insert((partial.is_class, setter_name)),
                        "already exisiting property"
                    );
                }
                if let Some(property) = partial.parse(data) {
                    methods.push(MethodOrProperty::Property(property));
                }
            }
            EntityKind::VisibilityAttr => {
                // Already exposed as entity.get_visibility()
            }
            EntityKind::TypeRef => {
                let name = entity.get_name().expect("typeref name");
                if let Some(Some(Some(GenericType { generics, .. }))) = &mut superclass {
                    generics.push(GenericType {
                        name,
                        generics: Vec::new(),
                    });
                } else {
                    panic!("unsupported typeref {entity:?}");
                }
            }
            EntityKind::ObjCException if superclass.is_some() => {
                // Maybe useful for knowing when to implement `Error` for the type
            }
            EntityKind::UnexposedAttr => {
                if let Some(macro_) = UnexposedMacro::parse(&entity) {
                    println!("objc decl {entity:?}: {macro_:?}");
                }
            }
            _ => panic!("unknown objc decl child {entity:?}"),
        };
        EntityVisitResult::Continue
    });

    if !properties.is_empty() {
        if properties == HashSet::from([(false, "setDisplayName".to_owned())]) {
            // TODO
        } else {
            panic!("did not properly add methods to properties:\n{methods:?}\n{properties:?}");
        }
    }

    (protocols, methods)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// @interface name: superclass <protocols*>
    ClassDecl {
        ty: GenericType,
        availability: Availability,
        superclass: Option<GenericType>,
        protocols: Vec<String>,
        methods: Vec<MethodOrProperty>,
        derives: Derives,
    },
    /// @interface class_name (name) <protocols*>
    CategoryDecl {
        class_ty: GenericType,
        availability: Availability,
        /// Some categories don't have a name. Example: NSClipView
        name: Option<String>,
        /// I don't quite know what this means?
        protocols: Vec<String>,
        methods: Vec<MethodOrProperty>,
    },
    /// @protocol name <protocols*>
    ProtocolDecl {
        name: String,
        availability: Availability,
        protocols: Vec<String>,
        methods: Vec<MethodOrProperty>,
    },
    /// struct name {
    ///     fields*
    /// };
    ///
    /// typedef struct {
    ///     fields*
    /// } name;
    ///
    /// typedef struct _name {
    ///     fields*
    /// } name;
    StructDecl {
        name: String,
        boxable: bool,
        fields: Vec<(String, Ty)>,
    },
    /// typedef NS_OPTIONS(type, name) {
    ///     variants*
    /// };
    ///
    /// typedef NS_ENUM(type, name) {
    ///     variants*
    /// };
    ///
    /// enum name {
    ///     variants*
    /// };
    ///
    /// enum {
    ///     variants*
    /// };
    EnumDecl {
        name: Option<String>,
        ty: Ty,
        kind: Option<UnexposedMacro>,
        variants: Vec<(String, Expr)>,
    },
    /// static const ty name = expr;
    /// extern const ty name;
    VarDecl {
        name: String,
        ty: Ty,
        value: Option<Expr>,
    },
    /// extern ret name(args*);
    ///
    /// static inline ret name(args*) {
    ///     body
    /// }
    FnDecl {
        name: String,
        arguments: Vec<(String, Ty)>,
        result_type: Ty,
        // Some -> inline function.
        body: Option<()>,
    },
    /// typedef Type TypedefName;
    AliasDecl { name: String, ty: Ty },
}

fn parse_struct(entity: &Entity<'_>, name: String) -> Stmt {
    let mut boxable = false;
    let mut fields = Vec::new();

    entity.visit_children(|entity, _parent| {
        match entity.get_kind() {
            EntityKind::UnexposedAttr => {
                if let Some(macro_) = UnexposedMacro::parse(&entity) {
                    panic!("unexpected attribute: {macro_:?}");
                }
            }
            EntityKind::FieldDecl => {
                let name = entity.get_name().expect("struct field name");
                let ty = entity.get_type().expect("struct field type");
                let ty = Ty::parse_struct_field(ty);

                if entity.is_bit_field() {
                    println!("[UNSOUND] struct bitfield {name}: {entity:?}");
                }

                fields.push((name, ty))
            }
            EntityKind::ObjCBoxable => {
                boxable = true;
            }
            _ => panic!("unknown struct field {entity:?}"),
        }
        EntityVisitResult::Continue
    });

    Stmt::StructDecl {
        name,
        boxable,
        fields,
    }
}

impl Stmt {
    pub fn parse(entity: &Entity<'_>, config: &Config) -> Option<Self> {
        match entity.get_kind() {
            // These are inconsequential for us, since we resolve imports differently
            EntityKind::ObjCClassRef | EntityKind::ObjCProtocolRef => None,
            EntityKind::ObjCInterfaceDecl => {
                // entity.get_mangled_objc_names()
                let name = entity.get_name().expect("class name");
                let class_data = config.class_data.get(&name);

                if class_data.map(|data| data.skipped).unwrap_or_default() {
                    return None;
                }

                let availability = Availability::parse(
                    entity
                        .get_platform_availability()
                        .expect("class availability"),
                );
                // println!("Availability: {:?}", entity.get_platform_availability());
                let mut superclass = None;
                let mut generics = Vec::new();

                let (protocols, methods) = parse_objc_decl(
                    &entity,
                    Some(&mut superclass),
                    Some(&mut generics),
                    class_data,
                );

                if let Some(new_name) =
                    class_data.and_then(|data| data.new_superclass_name.as_ref())
                {
                    match &mut superclass {
                        Some(Some(GenericType { name, .. })) => {
                            *name = new_name.clone();
                        }
                        _ => {
                            superclass = Some(Some(GenericType {
                                name: new_name.clone(),
                                generics: Vec::new(),
                            }))
                        }
                    }
                }

                let superclass = superclass.expect("no superclass found");

                Some(Self::ClassDecl {
                    ty: GenericType { name, generics },
                    availability,
                    superclass,
                    protocols,
                    methods,
                    derives: class_data
                        .map(|data| data.derives.clone())
                        .unwrap_or_default(),
                })
            }
            EntityKind::ObjCCategoryDecl => {
                let name = entity.get_name();
                let availability = Availability::parse(
                    entity
                        .get_platform_availability()
                        .expect("category availability"),
                );

                let mut class_name = None;
                entity.visit_children(|entity, _parent| {
                    if entity.get_kind() == EntityKind::ObjCClassRef {
                        if class_name.is_some() {
                            panic!("could not find unique category class")
                        }
                        class_name = Some(entity.get_name().expect("class name"));
                        EntityVisitResult::Break
                    } else {
                        EntityVisitResult::Continue
                    }
                });
                let class_name = class_name.expect("could not find category class");
                let class_data = config.class_data.get(&class_name);

                if class_data.map(|data| data.skipped).unwrap_or_default() {
                    return None;
                }

                let mut class_generics = Vec::new();

                let (protocols, methods) =
                    parse_objc_decl(&entity, None, Some(&mut class_generics), class_data);

                Some(Self::CategoryDecl {
                    class_ty: GenericType {
                        name: class_name,
                        generics: class_generics,
                    },
                    availability,
                    name,
                    protocols,
                    methods,
                })
            }
            EntityKind::ObjCProtocolDecl => {
                let name = entity.get_name().expect("protocol name");
                let protocol_data = config.protocol_data.get(&name);

                if protocol_data.map(|data| data.skipped).unwrap_or_default() {
                    return None;
                }

                let availability = Availability::parse(
                    entity
                        .get_platform_availability()
                        .expect("protocol availability"),
                );

                let (protocols, methods) = parse_objc_decl(&entity, None, None, protocol_data);

                Some(Self::ProtocolDecl {
                    name,
                    availability,
                    protocols,
                    methods,
                })
            }
            EntityKind::TypedefDecl => {
                let name = entity.get_name().expect("typedef name");
                let mut struct_ = None;
                let mut skip_struct = false;

                entity.visit_children(|entity, _parent| {
                    match entity.get_kind() {
                        // TODO: Parse NS_TYPED_EXTENSIBLE_ENUM vs. NS_TYPED_ENUM
                        EntityKind::UnexposedAttr => {
                            if let Some(macro_) = UnexposedMacro::parse(&entity) {
                                panic!("unexpected attribute: {macro_:?}");
                            }
                        }
                        EntityKind::StructDecl => {
                            if config
                                .struct_data
                                .get(&name)
                                .map(|data| data.skipped)
                                .unwrap_or_default()
                            {
                                skip_struct = true;
                                return EntityVisitResult::Continue;
                            }

                            let struct_name = entity.get_name();
                            if struct_name
                                .map(|name| name.starts_with('_'))
                                .unwrap_or(true)
                            {
                                // If this struct doesn't have a name, or the
                                // name is private, let's parse it with the
                                // typedef name.
                                struct_ = Some(parse_struct(&entity, name.clone()))
                            } else {
                                skip_struct = true;
                            }
                        }
                        EntityKind::ObjCClassRef
                        | EntityKind::ObjCProtocolRef
                        | EntityKind::TypeRef
                        | EntityKind::ParmDecl => {}
                        _ => panic!("unknown typedef child in {name}: {entity:?}"),
                    };
                    EntityVisitResult::Continue
                });

                if let Some(struct_) = struct_ {
                    return Some(struct_);
                }

                if skip_struct {
                    return None;
                }

                if config
                    .typedef_data
                    .get(&name)
                    .map(|data| data.skipped)
                    .unwrap_or_default()
                {
                    return None;
                }

                let ty = entity
                    .get_typedef_underlying_type()
                    .expect("typedef underlying type");
                Ty::parse_typedef(ty).map(|ty| Self::AliasDecl { name, ty })
            }
            EntityKind::StructDecl => {
                if let Some(name) = entity.get_name() {
                    if config
                        .struct_data
                        .get(&name)
                        .map(|data| data.skipped)
                        .unwrap_or_default()
                    {
                        return None;
                    }
                    if !name.starts_with('_') {
                        return Some(parse_struct(entity, name));
                    }
                }
                None
            }
            EntityKind::EnumDecl => {
                // Enum declarations show up twice for some reason, but
                // luckily this flag is set on the least descriptive entity.
                if !entity.is_definition() {
                    return None;
                }

                let name = entity.get_name();

                let data = config
                    .enum_data
                    .get(name.as_deref().unwrap_or("anonymous"))
                    .cloned()
                    .unwrap_or_default();
                if data.skipped {
                    return None;
                }

                let ty = entity.get_enum_underlying_type().expect("enum type");
                let is_signed = ty.is_signed_integer();
                let ty = Ty::parse_enum(ty);
                let mut kind = None;
                let mut variants = Vec::new();

                entity.visit_children(|entity, _parent| {
                    match entity.get_kind() {
                        EntityKind::EnumConstantDecl => {
                            let name = entity.get_name().expect("enum constant name");

                            if data
                                .constants
                                .get(&name)
                                .map(|data| data.skipped)
                                .unwrap_or_default()
                            {
                                return EntityVisitResult::Continue;
                            }

                            let val = Expr::from_val(
                                entity
                                    .get_enum_constant_value()
                                    .expect("enum constant value"),
                                is_signed,
                            );
                            let expr = if data.use_value {
                                val
                            } else {
                                Expr::parse_enum_constant(&entity).unwrap_or(val)
                            };
                            variants.push((name, expr));
                        }
                        EntityKind::UnexposedAttr => {
                            if let Some(macro_) = UnexposedMacro::parse(&entity) {
                                if let Some(kind) = &kind {
                                    assert_eq!(
                                        kind, &macro_,
                                        "got differing enum kinds in {name:?}"
                                    );
                                } else {
                                    kind = Some(macro_);
                                }
                            }
                        }
                        EntityKind::FlagEnum => {
                            let macro_ = UnexposedMacro::Options;
                            if let Some(kind) = &kind {
                                assert_eq!(kind, &macro_, "got differing enum kinds in {name:?}");
                            } else {
                                kind = Some(macro_);
                            }
                        }
                        _ => {
                            panic!("unknown enum child {entity:?} in {name:?}");
                        }
                    }
                    EntityVisitResult::Continue
                });

                Some(Self::EnumDecl {
                    name,
                    ty,
                    kind,
                    variants,
                })
            }
            EntityKind::VarDecl => {
                let name = entity.get_name().expect("var decl name");

                if config
                    .statics
                    .get(&name)
                    .map(|data| data.skipped)
                    .unwrap_or_default()
                {
                    return None;
                }

                let ty = entity.get_type().expect("var type");
                let ty = Ty::parse_static(ty);
                let mut value = None;

                entity.visit_children(|entity, _parent| {
                    match entity.get_kind() {
                        EntityKind::UnexposedAttr => {
                            if let Some(macro_) = UnexposedMacro::parse(&entity) {
                                panic!("unexpected attribute: {macro_:?}");
                            }
                        }
                        EntityKind::VisibilityAttr => {}
                        EntityKind::ObjCClassRef => {}
                        EntityKind::TypeRef => {}
                        _ if entity.is_expression() => {
                            if value.is_none() {
                                value = Some(Expr::parse_var(&entity));
                            } else {
                                panic!("got variable value twice")
                            }
                        }
                        _ => panic!("unknown vardecl child in {name}: {entity:?}"),
                    };
                    EntityVisitResult::Continue
                });

                let value = match value {
                    Some(Some(expr)) => Some(expr),
                    Some(None) => {
                        println!("skipped static {name}");
                        return None;
                    }
                    None => None,
                };

                Some(Self::VarDecl { name, ty, value })
            }
            EntityKind::FunctionDecl => {
                let name = entity.get_name().expect("function name");

                if config
                    .fns
                    .get(&name)
                    .map(|data| data.skipped)
                    .unwrap_or_default()
                {
                    return None;
                }

                if entity.is_variadic() {
                    println!("can't handle variadic function {name}");
                    return None;
                }

                let result_type = entity.get_result_type().expect("function result type");
                let result_type = Ty::parse_function_return(result_type);
                let mut arguments = Vec::new();

                assert!(
                    !entity.is_static_method(),
                    "unexpected static method {name}"
                );

                entity.visit_children(|entity, _parent| {
                    match entity.get_kind() {
                        EntityKind::UnexposedAttr => {
                            if let Some(macro_) = UnexposedMacro::parse(&entity) {
                                panic!("unexpected function attribute: {macro_:?}");
                            }
                        }
                        EntityKind::ObjCClassRef | EntityKind::TypeRef => {}
                        EntityKind::ParmDecl => {
                            // Could also be retrieved via. `get_arguments`
                            let name = entity.get_name().unwrap_or_else(|| "_".into());
                            let ty = entity.get_type().expect("function argument type");
                            let ty = Ty::parse_function_argument(ty);
                            arguments.push((name, ty))
                        }
                        _ => panic!("unknown function child in {name}: {entity:?}"),
                    };
                    EntityVisitResult::Continue
                });

                let body = if entity.is_inline_function() {
                    Some(())
                } else {
                    None
                };

                Some(Self::FnDecl {
                    name,
                    arguments,
                    result_type,
                    body,
                })
            }
            EntityKind::UnionDecl => {
                // println!(
                //     "union: {:?}, {:?}, {:#?}, {:#?}",
                //     entity.get_display_name(),
                //     entity.get_name(),
                //     entity.has_attributes(),
                //     entity.get_children(),
                // );
                None
            }
            _ => {
                panic!("Unknown: {:?}", entity)
            }
        }
    }

    pub fn compare(&self, other: &Self) {
        if self != other {
            match (&self, &other) {
                (
                    Self::ClassDecl {
                        methods: self_methods,
                        ..
                    },
                    Self::ClassDecl {
                        methods: other_methods,
                        ..
                    },
                ) => {
                    super::compare_vec(
                        &self_methods,
                        &other_methods,
                        |_i, self_method, other_method| {
                            assert_eq!(self_method, other_method, "methods were not equal");
                        },
                    );
                }
                _ => {}
            }

            panic!("statements were not equal:\n{self:#?}\n{other:#?}");
        }
    }
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct GenericTyHelper<'a>(&'a GenericType);

        impl fmt::Display for GenericTyHelper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.name)?;
                if !self.0.generics.is_empty() {
                    write!(f, "<")?;
                    for generic in &self.0.generics {
                        write!(f, "{generic}, ")?;
                    }
                    for generic in &self.0.generics {
                        write!(f, "{generic}Ownership, ")?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
        }

        struct GenericParamsHelper<'a>(&'a [GenericType]);

        impl fmt::Display for GenericParamsHelper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if !self.0.is_empty() {
                    write!(f, "<")?;
                    for generic in self.0 {
                        write!(f, "{generic}: Message, ")?;
                    }
                    for generic in self.0 {
                        write!(f, "{generic}Ownership: Ownership, ")?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
        }

        match self {
            Self::ClassDecl {
                ty,
                availability: _,
                superclass,
                protocols: _,
                methods,
                derives,
            } => {
                let default_superclass = GenericType {
                    name: "Object".into(),
                    generics: Vec::new(),
                };
                let superclass = superclass.as_ref().unwrap_or_else(|| &default_superclass);

                // TODO: Use ty.get_objc_protocol_declarations()

                let macro_name = if ty.generics.is_empty() {
                    "extern_class"
                } else {
                    "__inner_extern_class"
                };

                writeln!(f, "{macro_name}!(")?;
                writeln!(f, "    {}", derives)?;
                write!(f, "    pub struct ")?;
                if ty.generics.is_empty() {
                    write!(f, "{}", ty.name)?;
                } else {
                    write!(f, "{}<", ty.name)?;
                    for generic in &ty.generics {
                        write!(f, "{generic}: Message = Object, ")?;
                    }
                    for generic in &ty.generics {
                        write!(f, "{generic}Ownership: Ownership = Shared, ")?;
                    }
                    write!(f, ">")?;
                };
                if ty.generics.is_empty() {
                    writeln!(f, ";")?;
                } else {
                    writeln!(f, " {{")?;
                    for (i, generic) in ty.generics.iter().enumerate() {
                        // Invariant over the generic (for now)
                        writeln!(
                            f,
                            "_inner{i}: PhantomData<*mut ({generic}, {generic}Ownership)>,"
                        )?;
                    }
                    writeln!(f, "notunwindsafe: PhantomData<&'static mut ()>,")?;
                    writeln!(f, "}}")?;
                }
                writeln!(f, "")?;
                writeln!(
                    f,
                    "    unsafe impl{} ClassType for {} {{",
                    GenericParamsHelper(&ty.generics),
                    GenericTyHelper(&ty)
                )?;
                writeln!(f, "        type Super = {};", GenericTyHelper(&superclass))?;
                writeln!(f, "    }}")?;
                writeln!(f, ");")?;
                writeln!(f, "")?;
                writeln!(f, "extern_methods!(")?;
                writeln!(
                    f,
                    "    unsafe impl{} {} {{",
                    GenericParamsHelper(&ty.generics),
                    GenericTyHelper(&ty)
                )?;
                for method in methods {
                    writeln!(f, "{method}")?;
                }
                writeln!(f, "    }}")?;
                writeln!(f, ");")?;
            }
            Self::CategoryDecl {
                class_ty,
                availability: _,
                name,
                protocols: _,
                methods,
            } => {
                writeln!(f, "extern_methods!(")?;
                if let Some(name) = name {
                    writeln!(f, "    /// {name}")?;
                }
                writeln!(
                    f,
                    "    unsafe impl{} {} {{",
                    GenericParamsHelper(&class_ty.generics),
                    GenericTyHelper(&class_ty)
                )?;
                for method in methods {
                    writeln!(f, "{method}")?;
                }
                writeln!(f, "    }}")?;
                writeln!(f, ");")?;
            }
            Self::ProtocolDecl {
                name,
                availability: _,
                protocols: _,
                methods,
            } => {
                writeln!(f, "extern_protocol!(")?;
                writeln!(f, "    pub struct {name};")?;
                writeln!(f, "")?;
                writeln!(f, "    unsafe impl ProtocolType for {name} {{")?;
                for method in methods {
                    writeln!(f, "{method}")?;
                }
                writeln!(f, "    }}")?;
                writeln!(f, ");")?;
            }
            Self::StructDecl {
                name,
                boxable: _,
                fields,
            } => {
                writeln!(f, "extern_struct!(")?;
                writeln!(f, "    pub struct {name} {{")?;
                for (name, ty) in fields {
                    write!(f, "        ")?;
                    if !name.starts_with('_') {
                        write!(f, "pub ")?;
                    }
                    writeln!(f, "{name}: {ty},")?;
                }
                writeln!(f, "    }}")?;
                writeln!(f, ");")?;
            }
            Self::EnumDecl {
                name,
                ty,
                kind,
                variants,
            } => {
                let macro_name = match kind {
                    None => "extern_enum",
                    Some(UnexposedMacro::Enum) => "ns_enum",
                    Some(UnexposedMacro::Options) => "ns_options",
                    Some(UnexposedMacro::ClosedEnum) => "ns_closed_enum",
                    Some(UnexposedMacro::ErrorEnum) => "ns_error_enum",
                };
                writeln!(f, "{}!(", macro_name)?;
                writeln!(f, "    #[underlying({ty})]")?;
                write!(f, "    pub enum ",)?;
                if let Some(name) = name {
                    write!(f, "{name} ")?;
                }
                writeln!(f, "{{")?;
                for (name, expr) in variants {
                    writeln!(f, "        {name} = {expr},")?;
                }
                writeln!(f, "    }}")?;
                writeln!(f, ");")?;
            }
            Self::VarDecl {
                name,
                ty,
                value: None,
            } => {
                writeln!(f, "extern_static!({name}: {ty});")?;
            }
            Self::VarDecl {
                name,
                ty,
                value: Some(expr),
            } => {
                writeln!(f, "extern_static!({name}: {ty} = {expr});")?;
            }
            Self::FnDecl {
                name,
                arguments,
                result_type,
                body: None,
            } => {
                writeln!(f, "extern_fn!(")?;
                write!(f, "    pub unsafe fn {name}(")?;
                for (param, arg_ty) in arguments {
                    write!(f, "{}: {arg_ty},", handle_reserved(&param))?;
                }
                writeln!(f, "){result_type};")?;
                writeln!(f, ");")?;
            }
            Self::FnDecl {
                name,
                arguments,
                result_type,
                body: Some(_body),
            } => {
                writeln!(f, "inline_fn!(")?;
                write!(f, "    pub unsafe fn {name}(")?;
                for (param, arg_ty) in arguments {
                    write!(f, "{}: {arg_ty},", handle_reserved(&param))?;
                }
                writeln!(f, "){result_type} {{")?;
                writeln!(f, "        todo!()")?;
                writeln!(f, "    }}")?;
                writeln!(f, ");")?;
            }
            Self::AliasDecl { name, ty } => {
                writeln!(f, "pub type {name} = {ty};")?;
            }
        };
        Ok(())
    }
}

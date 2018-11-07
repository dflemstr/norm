use std::collections;

use specs;

use ir::component::element;
use ir::component::ty;
use std::ops;

pub struct InferTypesSystem;

impl<'a> specs::System<'a> for InferTypesSystem {
    type SystemData = (
        specs::Entities<'a>,
        specs::ReadStorage<'a, element::Element>,
        specs::WriteStorage<'a, ty::Type>,
    );

    fn run(&mut self, (entities, elements, mut types): Self::SystemData) {
        use specs::prelude::ParallelIterator;
        use specs::Join;
        use specs::ParJoin;

        loop {
            for (entity, element) in (&entities, &elements).join() {
                debug!("{:?} has element {:?}", entity, element);
            }

            let new_types = (&entities, &elements, !&types)
                .par_join()
                .flat_map(|(entity, element, _)| infer_type(element, &types).map(|ty| (entity, ty)))
                .collect::<Vec<_>>();
            debug!("inferred new types: {:?}", new_types);
            if new_types.is_empty() {
                break;
            }

            for (entity, ty) in new_types {
                types.insert(entity, ty).unwrap();
            }
        }
    }
}

fn infer_type<D>(
    element: &element::Element,
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    match *element {
        element::Element::Number(_) => Some(ty::Type::Number),
        element::Element::String(_) => Some(ty::Type::String),
        element::Element::Tuple { ref fields } => infer_tuple_type(fields, types),
        element::Element::Record { ref fields } => infer_record_type(fields, types),
        element::Element::Reference(_) => None,
        element::Element::Select { record, ref field } => infer_select_type(record, field, types),
        element::Element::Apply {
            function,
            ref parameters,
        } => infer_apply_type(function, parameters, types),
        element::Element::Parameter {
            name: _,
            signature,
        } => infer_parameter_type( signature, types),
        element::Element::Capture { name: _, captured } => infer_capture_type(captured, types),
        element::Element::Closure {
            captures: _,
            ref parameters,
            statements: _,
            signature,
        } => infer_closure_type(parameters, signature, types),
        element::Element::Module { ref definitions } => infer_module_type(definitions, types),
    }
}

fn infer_tuple_type<D>(
    fields: &[specs::Entity],
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    fields
        .iter()
        .map(|f| types.get(*f).cloned())
        .collect::<Option<Vec<_>>>()
        .map(|fields| ty::Type::Tuple(ty::Tuple { fields }))
}

fn infer_record_type<D>(
    fields: &collections::HashMap<String, specs::Entity>,
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    fields
        .iter()
        .map(|(k, v)| types.get(*v).map(|t| (k.clone(), t.clone())))
        .collect::<Option<collections::HashMap<_, _>>>()
        .map(|fields| ty::Type::Record(ty::Record { fields }))
}

fn infer_select_type<D>(
    record: specs::Entity,
    field: &str,
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    match types.get(record) {
        None => None,
        Some(t) => match t {
            ty::Type::Record(ty::Record { ref fields }) => {
                if let Some(t) = fields.get(field) {
                    Some(t.clone())
                } else {
                    let mut expected_fields = collections::HashMap::new();
                    expected_fields.insert(field.to_owned(), ty::Type::Any);
                    Some(ty::Type::Conflict(ty::Conflict {
                        expected: Box::new(ty::Type::Record(ty::Record {
                            fields: expected_fields,
                        })),
                        actual: Box::new(t.clone()),
                    }))
                }
            }
            something => {
                let mut expected_fields = collections::HashMap::new();
                expected_fields.insert(field.to_owned(), ty::Type::Any);
                Some(ty::Type::Conflict(ty::Conflict {
                    expected: Box::new(ty::Type::Record(ty::Record {
                        fields: expected_fields,
                    })),
                    actual: Box::new(something.clone()),
                }))
            }
        },
    }
}

fn infer_apply_type<D>(
    function: specs::Entity,
    parameters: &[specs::Entity],
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    match types.get(function) {
        None => {
            trace!("inference failure: missing function type for apply");
            None
        }
        Some(f) => match f {
            ty::Type::Function(ty::Function {
                parameters: ref formal_parameters,
                result,
            }) => {
                if let Some(parameters) = parameters
                    .iter()
                    .map(|p| types.get(*p).cloned())
                    .collect::<Option<Vec<_>>>()
                {
                    if parameters == *formal_parameters {
                        Some((**result).clone())
                    } else {
                        Some(ty::Type::Conflict(ty::Conflict {
                            expected: Box::new(ty::Type::Function(ty::Function {
                                parameters,
                                result: Box::new(ty::Type::Any),
                            })),
                            actual: Box::new(f.clone()),
                        }))
                    }
                } else {
                    None
                }
            }
            something => Some(ty::Type::Conflict(ty::Conflict {
                expected: Box::new(ty::Type::Function(ty::Function {
                    parameters: vec![ty::Type::Any],
                    result: Box::new(ty::Type::Any),
                })),
                actual: Box::new(something.clone()),
            })),
        },
    }
}

fn infer_parameter_type<D>(
    signature: Option<specs::Entity>,
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    if let Some(signature) = signature {
        types.get(signature).cloned()
    } else {
        trace!("inference failure: no signature for parameter");
        // TODO: implement surjective type inference
        None
    }
}

fn infer_capture_type<D>(
    capture: specs::Entity,
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    types.get(capture).cloned()
}

fn infer_closure_type<D>(
    parameters: &[specs::Entity],
    signature: Option<specs::Entity>,
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    if let Some(ref signature) = signature {
        if let Some(result) = types.get(*signature).cloned() {
            if let Some(parameters) = parameters
                .iter()
                .map(|p| types.get(*p).cloned())
                .collect::<Option<Vec<_>>>()
            {
                let result = Box::new(result);
                Some(ty::Type::Function(ty::Function { parameters, result }))
            } else {
                trace!("inference failure: missing parameter type(s) for closure");
                None
            }
        } else {
            trace!("inference failure: missing signature type for closure");
            None
        }
    } else {
        trace!("inference failure: no signature for closure");
        // TODO: implement surjective type inference
        None
    }
}

fn infer_module_type<D>(
    definitions: &collections::HashMap<String, specs::Entity>,
    types: &specs::Storage<ty::Type, D>,
) -> Option<ty::Type>
where
    D: ops::Deref<Target = specs::storage::MaskedStorage<ty::Type>>,
{
    definitions
        .iter()
        .map(|(k, v)| types.get(*v).map(|t| (k.clone(), t.clone())))
        .collect::<Option<collections::HashMap<_, _>>>()
        .map(|fields| ty::Type::Record(ty::Record { fields }))
}
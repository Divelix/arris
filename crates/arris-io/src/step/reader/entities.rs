//! Instances by the schema's types: references resolved, parameters read
//! as the types the schema gives them. Every mismatch is a
//! [`Refusal::Malformed`] naming the instance it was found in, so a
//! malformed file is refused where it is wrong and never panics.

use std::collections::BTreeMap;

use crate::step::part21::{Instance, Param, Record};

use super::Refusal;

/// The instances of a parsed file, looked up by id.
#[derive(Clone, Copy)]
pub(crate) struct Entities<'a> {
    instances: &'a BTreeMap<u64, Instance>,
}

/// One record of an instance, with the instance's id for its refusals.
#[derive(Clone, Copy)]
pub(crate) struct Args<'a> {
    /// The instance the record belongs to.
    pub(crate) id: u64,
    /// The record.
    pub(crate) record: &'a Record,
}

/// A refusal of `entity` for being malformed.
pub(crate) fn malformed(entity: u64, what: impl Into<String>) -> Refusal {
    Refusal::Malformed {
        entity,
        what: what.into(),
    }
}

impl<'a> Entities<'a> {
    pub(crate) fn new(instances: &'a BTreeMap<u64, Instance>) -> Self {
        Entities { instances }
    }

    /// Instance `id`, referenced from `from`. Errors: `id` is not defined,
    /// named against `from`, the instance that refers to it.
    pub(crate) fn get(&self, from: u64, id: u64) -> Result<&'a Instance, Refusal> {
        self.instances.get(&id).ok_or_else(|| {
            malformed(
                from,
                format!("refers to #{id}, which the file does not define"),
            )
        })
    }

    /// The record named `name` of instance `id`, referenced from `from`.
    /// Errors: as [`Entities::get`], or the instance has no such record.
    pub(crate) fn record(&self, from: u64, id: u64, name: &str) -> Result<Args<'a>, Refusal> {
        let instance = self.get(from, id)?;
        let record = instance.record(name).ok_or_else(|| {
            malformed(
                id,
                format!("is {}, where {name} belongs", describe(instance)),
            )
        })?;
        Ok(Args { id, record })
    }
}

/// An instance's entity names, for a refusal: `NAME` or `(A B C)`.
pub(crate) fn describe(instance: &Instance) -> String {
    match instance {
        Instance::Simple(r) => r.name.clone(),
        Instance::Complex(rs) => format!(
            "({})",
            rs.iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

/// A real, from a real or an integer, through any typed wrapper
/// (`LENGTH_MEASURE(2.5)`): the value a measure parameter carries.
pub(crate) fn number(p: &Param) -> Option<f64> {
    match p {
        Param::Real(x) => Some(*x),
        // An integer where a real belongs is what some writers put for a
        // whole number; the grammar tells them apart, the value does not.
        Param::Integer(i) => Some(*i as f64),
        Param::Typed(_, inner) => number(inner),
        _ => None,
    }
}

impl<'a> Args<'a> {
    fn param(&self, i: usize) -> Result<&'a Param, Refusal> {
        self.record.params.get(i).ok_or_else(|| {
            malformed(
                self.id,
                format!(
                    "{} has {} parameters, fewer than {}",
                    self.record.name,
                    self.record.params.len(),
                    i + 1
                ),
            )
        })
    }

    fn wrong(&self, i: usize, what: &str) -> Refusal {
        malformed(
            self.id,
            format!("{} parameter {} is not {what}", self.record.name, i + 1),
        )
    }

    /// The number of parameters.
    pub(crate) fn len(&self) -> usize {
        self.record.params.len()
    }

    /// Parameter `i` as a real.
    pub(crate) fn real(&self, i: usize) -> Result<f64, Refusal> {
        number(self.param(i)?).ok_or_else(|| self.wrong(i, "a number"))
    }

    /// Parameter `i` as a reference.
    pub(crate) fn reference(&self, i: usize) -> Result<u64, Refusal> {
        match self.param(i)? {
            Param::Ref(id) => Ok(*id),
            _ => Err(self.wrong(i, "a reference")),
        }
    }

    /// Parameter `i` as a reference, or `None` for `$`.
    pub(crate) fn optional_reference(&self, i: usize) -> Result<Option<u64>, Refusal> {
        match self.param(i)? {
            Param::Ref(id) => Ok(Some(*id)),
            Param::Unset => Ok(None),
            _ => Err(self.wrong(i, "a reference or `$`")),
        }
    }

    /// Parameter `i` as an enumeration value, upper-cased, or `None` for
    /// `$`.
    pub(crate) fn optional_enumeration(&self, i: usize) -> Result<Option<&'a str>, Refusal> {
        match self.param(i)? {
            Param::Enumeration(e) => Ok(Some(e.as_str())),
            Param::Unset => Ok(None),
            _ => Err(self.wrong(i, "an enumeration")),
        }
    }

    /// Parameter `i` as an enumeration value, upper-cased.
    pub(crate) fn enumeration(&self, i: usize) -> Result<&'a str, Refusal> {
        self.optional_enumeration(i)?
            .ok_or_else(|| self.wrong(i, "an enumeration"))
    }

    /// Parameter `i` as a list.
    pub(crate) fn list(&self, i: usize) -> Result<&'a [Param], Refusal> {
        match self.param(i)? {
            Param::List(items) => Ok(items),
            _ => Err(self.wrong(i, "a list")),
        }
    }

    /// Parameter `i` as a list of references.
    pub(crate) fn references(&self, i: usize) -> Result<Vec<u64>, Refusal> {
        self.list(i)?
            .iter()
            .map(|p| match p {
                Param::Ref(id) => Ok(*id),
                _ => Err(self.wrong(i, "a list of references")),
            })
            .collect()
    }

    /// Parameter `i` as a list of numbers.
    pub(crate) fn reals(&self, i: usize) -> Result<Vec<f64>, Refusal> {
        self.list(i)?
            .iter()
            .map(|p| number(p).ok_or_else(|| self.wrong(i, "a list of numbers")))
            .collect()
    }

    /// A refusal of this instance for being malformed.
    pub(crate) fn malformed(&self, what: impl Into<String>) -> Refusal {
        malformed(self.id, what)
    }
}

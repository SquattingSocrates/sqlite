use std::collections::HashMap;
use std::convert::TryFrom;
use std::ops::{Deref, Index};
use std::rc::Rc;

use error::{Error, Result};
use statement::{Bindable, State, Statement};
use value::Value;

/// An iterator for a prepared statement.
pub struct Cursor<'m> {
    statement: &'m mut Statement,
    values: Vec<Value>,
}

/// An iterator for a prepared statement with ownership.
pub struct CursorWithOwnership {
    statement: Statement,
    values: Vec<Value>,
}

/// A row.
#[derive(Debug)]
pub struct Row {
    column_mapping: Rc<HashMap<String, usize>>,
    values: Vec<Value>,
}

/// A type suitable for indexing columns in a row.
pub trait RowIndex: std::fmt::Debug {
    /// Identify the ordinal position.
    ///
    /// The first column has index 0.
    fn index(self, row: &Row) -> usize;
}

macro_rules! implement(
    ($type:ident<$($lifetime:lifetime),+>) => {
        impl<$($lifetime),+> $type<$($lifetime),+> {
            /// Bind values to parameters.
            ///
            /// In case of integer indices, the first parameter has index 1. See
            /// `Statement::bind` for further details.
            pub fn bind<T: Bindable>(self, value: T) -> Result<Self> {
                #[allow(unused_mut)]
                let mut cursor = self.reset()?;
                cursor.statement.bind(value)?;
                Ok(cursor)
            }

            /// Bind values to parameters via an iterator.
            ///
            /// See `Statement::bind_iter` for further details.
            #[allow(unused_mut)]
            pub fn bind_iter<T, U>(self, value: T) -> Result<Self>
            where
                T: IntoIterator<Item = U>,
                U: Bindable,
            {
                let mut cursor = self.reset()?;
                cursor.statement.bind_iter(value)?;
                Ok(cursor)
            }

            /// Reset the internal state.
            #[allow(unused_mut)]
            pub fn reset(mut self) -> Result<Self> {
                self.statement.reset()?;
                Ok(self)
            }

            /// Advance to the next row and read all columns.
            pub fn try_next(&mut self) -> Result<Option<&[Value]>> {
                if self.statement.next()? == State::Done {
                    return Ok(None);
                }
                for (index, value) in self.values.iter_mut().enumerate() {
                    *value = self.statement.read(index)?;
                }
                Ok(Some(&self.values))
            }
        }

        impl<$($lifetime),+> Deref for $type<$($lifetime),+> {
            type Target = Statement;

            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.statement
            }
        }

        impl<$($lifetime),+> Iterator for $type<$($lifetime),+> {
            type Item = Result<Row>;

            fn next(&mut self) -> Option<Self::Item> {
                let column_mapping = self.statement.column_mapping();
                self.try_next()
                    .map(|row| {
                        row.map(|row| Row {
                            column_mapping: column_mapping,
                            values: row.to_vec(),
                        })
                    })
                    .transpose()
            }
        }
    };
    ($type:ident) => {
        impl$type {
            /// Bind values to parameters.
            ///
            /// See `Statement::bind` for further details.
            pub fn bind<T: Bindable>(self, value: T) -> Result<Self> {
                #[allow(unused_mut)]
                let mut cursor = self.reset()?;
                cursor.statement.bind(value)?;
                Ok(cursor)
            }

            /// Bind values to parameters via an iterator.
            ///
            /// See `Statement::bind_iter` for further details.
            pub fn bind_iter<T, U>(self, value: T) -> Result<Self>
            where
                T: IntoIterator<Item = U>,
                U: Bindable,
            {
                #[allow(unused_mut)]
                let mut cursor = self.reset()?;
                cursor.statement.bind_iter(value)?;
                Ok(cursor)
            }

            /// Reset the internal state.
            pub fn reset(mut self) -> Result<Self> {
                self.statement.reset()?;
                Ok(self)
            }

            /// Advance to the next row and read all columns.
            pub fn try_next(&mut self) -> Result<Option<&[Value]>> {
                if self.statement.next()? == State::Done {
                    return Ok(None);
                }
                for (index, value) in self.values.iter_mut().enumerate() {
                    *value = self.statement.read(index)?;
                }
                Ok(Some(&self.values))
            }
        }

        impl Deref for $type {
            type Target = Statement;

            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.statement
            }
        }

        impl Iterator for $type {
            type Item = Result<Row>;

            fn next(&mut self) -> Option<Self::Item> {
                let column_mapping = self.statement.column_mapping();
                self.try_next()
                    .map(|row| {
                        row.map(|row| Row {
                            column_mapping: column_mapping,
                            values: row.to_vec(),
                        })
                    })
                    .transpose()
            }
        }
    }
);

implement!(Cursor<'m>);
implement!(CursorWithOwnership);

impl<'l> From<CursorWithOwnership> for Statement {
    #[inline]
    fn from(cursor: CursorWithOwnership) -> Self {
        cursor.statement
    }
}

impl Row {
    /// Read the value in a column.
    ///
    /// In case of integer indices, the first column has index 0.
    ///
    /// # Panics
    ///
    /// Panics if the column could not be read.
    #[inline]
    pub fn read<'l, T, U>(&'l self, column: U) -> T
    where
        T: TryFrom<&'l Value, Error = Error>,
        U: RowIndex,
    {
        self.try_read(column).unwrap()
    }

    /// Try to read the value in a column.
    ///
    /// In case of integer indices, the first column has index 0.
    #[inline]
    pub fn try_read<'l, T, U>(&'l self, column: U) -> Result<T>
    where
        T: TryFrom<&'l Value, Error = Error>,
        U: RowIndex,
    {
        T::try_from(&self.values[column.index(self)])
    }
}

impl From<Row> for Vec<Value> {
    #[inline]
    fn from(row: Row) -> Self {
        row.values
    }
}

impl<T> Index<T> for Row
where
    T: RowIndex,
{
    type Output = Value;

    fn index(&self, index: T) -> &Value {
        &self.values[index.index(self)]
    }
}

impl RowIndex for &str {
    #[inline]
    fn index(self, row: &Row) -> usize {
        debug_assert!(
            row.column_mapping.contains_key(self),
            "the index is out of range"
        );
        row.column_mapping[self]
    }
}

impl RowIndex for usize {
    #[inline]
    fn index(self, row: &Row) -> usize {
        debug_assert!(self < row.values.len(), "the index is out of range");
        self
    }
}

pub fn new<'l, 'm>(statement: &'m mut Statement) -> Cursor<'m> {
    let values = vec![Value::Null; statement.column_count()];
    Cursor {
        statement: statement,
        values: values,
    }
}

pub fn new_with_ownership<'l>(statement: Statement) -> CursorWithOwnership {
    let values = vec![Value::Null; statement.column_count()];
    CursorWithOwnership {
        statement: statement,
        values: values,
    }
}

use std::{cmp::Ordering::*, fmt};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Number(f64),
    String(String),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Cmp {
    Lt,
    Le,
    Eq,
    Ne,
    Gt,
    Ge,
}

impl From<bool> for Value {
    fn from(cond: bool) -> Self {
        if cond {
            Self::Number(1.0)
        } else {
            Self::Null
        }
    }
}

impl Value {
    pub(crate) fn bool(&self) -> bool {
        !matches!(self, Value::Null)
    }

    pub(crate) fn num(&self, unit: i8) -> f64 {
        match self {
            Value::Number(n) => *n,
            Value::String(_) => 1.0,
            Value::Null => unit as f64,
        }
    }

    pub(crate) fn str_to(&self, buf: &mut String) {
        use fmt::Write;
        match self {
            Value::Number(n) => write!(buf, "{n}").unwrap(),
            Value::String(s) => buf.push_str(s),
            Value::Null => (),
        }
    }

    pub(crate) fn str(self) -> String {
        match self {
            Value::Number(n) => n.to_string(),
            Value::String(s) => s,
            Value::Null => String::new(),
        }
    }

    pub(crate) fn apply_neg(&mut self) {
        match self {
            Value::Number(n) => *n = -*n,
            Value::String(s) => {
                let mut new = String::with_capacity(s.len());
                new.extend(s.chars().rev());
                *s = new;
            },
            Value::Null => (),
        }
    }

    pub(crate) fn apply_not(&mut self) {
        *self = self.bool().into()
    }

    pub(crate) fn apply_add(&mut self, rhs: Self) {
        match self {
            Value::Number(n) => *n += rhs.num(0),
            Value::String(s) => rhs.str_to(s),
            Value::Null => *self = rhs,
        }
    }

    pub(crate) fn apply_sub(&mut self, rhs: Self) {
        match self {
            Value::Number(n) => *n -= rhs.num(0),
            Value::String(s) => {
                let pat = rhs.str();
                for i in 0..s.len() {
                    while let Some(rest) = s.get(i..)
                        && rest.starts_with(&pat)
                    {
                        s.drain(i..i+pat.len());
                    }
                }
            },
            Value::Null => *self = rhs,
        }
    }

    pub(crate) fn apply_mul(&mut self, rhs: Self) {
        match self {
            Value::Number(n) => *n *= rhs.num(1),
            Value::String(s) => {
                let count = rhs.num(0).floor();
                if count <= 0.0 {
                    s.clear();
                } else {
                    let basic = s.len();
                    for _ in 1..count as u64 {
                        s.extend_from_within(..basic);
                    }
                }
            },
            Value::Null => *self = rhs,
        }
    }

    pub(crate) fn apply_div(&mut self, rhs: Self) {
        match self {
            Value::Number(n) => *n /= rhs.num(1),
            Value::String(s) => {
                let count = rhs.num(0).floor() as usize;
                let new_len = s.char_indices().nth(count)
                    .map_or(s.len(), |it| it.0);
                s.truncate(new_len);
            },
            Value::Null => *self = rhs,
        }
    }

    pub(crate) fn apply_rem(&mut self, rhs: Self) {
        match self {
            Value::Number(n) => *n %= rhs.num(1),
            Value::String(s) => {
                let count = rhs.num(0).floor() as usize;
                let new_len = s.char_indices().nth(count)
                    .map_or(s.len(), |it| it.0);
                s.drain(..new_len);
            },
            Value::Null => *self = Self::Number(0.0),
        }
    }

    pub(crate) fn apply_cmp(&mut self, rhs: Self, op: Cmp) {
        let cmp = Ord::cmp(self, &rhs);
        let cond = match op {
            Cmp::Lt => cmp.is_lt(),
            Cmp::Le => cmp.is_le(),
            Cmp::Eq => cmp.is_eq(),
            Cmp::Ne => cmp.is_ne(),
            Cmp::Gt => cmp.is_gt(),
            Cmp::Ge => cmp.is_ge(),
        };
        *self = cond.into();
    }

    pub(crate) fn apply_replace(&mut self, rhs: Self) {
        *self = rhs;
    }
}

impl Eq for Value {}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a.total_cmp(b),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Number(_), Value::String(_)) => Less,
            (Value::String(_), Value::Number(_)) => Greater,
            (Value::Null, Value::Null) => Equal,
            (_, Value::Null) => Greater,
            (Value::Null, _) => Less,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Null => write!(f, "NULL"),
        }
    }
}

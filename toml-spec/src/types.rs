use {
    error_reporter::Report,
    indexmap::IndexMap,
    serde::{
        Deserialize, Deserializer,
        de::{DeserializeOwned, Error},
    },
};

#[derive(Debug, Deserialize)]
pub struct Described<T> {
    pub description: String,
    #[serde(flatten)]
    pub value: T,
}

#[derive(Debug)]
pub enum TopLevelTypeSpec {
    Variable {
        variants: Vec<Described<VariantSpec>>,
    },
    Single(VariantSpec),
}

#[derive(Debug)]
pub enum TableSpec {
    Tagged {
        types: IndexMap<String, Described<SingleTableSpec>>,
    },
    Single(SingleTableSpec),
}

#[derive(Debug, Deserialize)]
pub struct SingleTableSpec {
    pub fields: IndexMap<String, Described<TableFieldSpec>>,
}

#[derive(Debug, Deserialize)]
pub struct TableFieldSpec {
    pub required: bool,
    #[serde(flatten)]
    pub kind: RefOrSpec<NestableTypesSpec>,
}

#[derive(Debug)]
pub enum RefOrSpec<T> {
    Ref { name: String },
    Spec(T),
}

#[derive(Debug, Deserialize)]
pub struct StringSpec {
    pub pattern: Option<String>,
    pub values: Option<Vec<Described<StringSpecValue>>>,
}

#[derive(Debug, Deserialize)]
pub struct StringSpecValue {
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct NumberSpec {
    #[serde(default)]
    pub integer_only: bool,
    pub minimum: Option<f64>,
    #[serde(default)]
    pub exclusive_minimum: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum VariantSpec {
    String(RefOrSpec<StringSpec>),
    Number(RefOrSpec<NumberSpec>),
    Boolean,
    Array(RefOrSpec<ArraySpec>),
    Table(RefOrSpec<TableSpec>),
}

#[derive(Debug, Deserialize)]
pub struct ArraySpec {
    pub items: Box<RefOrSpec<NestableTypesSpec>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum NestableTypesSpec {
    String(StringSpec),
    Number(NumberSpec),
    Boolean,
    Array(ArraySpec),
    Map(MapSpec),
}

#[derive(Debug, Deserialize)]
pub struct MapSpec {
    pub values: Box<RefOrSpec<NestableTypesSpec>>,
}

impl<'de> Deserialize<'de> for TopLevelTypeSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = serde_yaml::Value::deserialize(deserializer)?;
        #[derive(Debug, Deserialize)]
        struct Variable {
            variants: Vec<Described<VariantSpec>>,
        }
        let variable = Variable::deserialize(&v);
        let single = VariantSpec::deserialize(&v);
        let res = match (variable, single) {
            (Ok(variable), _) => Self::Variable {
                variants: variable.variants,
            },
            (_, Ok(single)) => Self::Single(single),
            (Err(e1), Err(e2)) => {
                return Err(Error::custom(format!(
                    "spec must define either variants or a single variant. failures: {} ----- {}",
                    Report::new(e1),
                    Report::new(e2)
                )));
            }
        };
        Ok(res)
    }
}

impl<'de> Deserialize<'de> for TableSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = serde_yaml::Value::deserialize(deserializer)?;
        #[derive(Debug, Deserialize)]
        struct Tagged {
            types: IndexMap<String, Described<SingleTableSpec>>,
        }
        let tagged = Tagged::deserialize(&v);
        let single = SingleTableSpec::deserialize(&v);
        let res = match (tagged, single) {
            (Ok(tagged), _) => Self::Tagged {
                types: tagged.types,
            },
            (_, Ok(single)) => Self::Single(single),
            (Err(e1), Err(e2)) => {
                return Err(Error::custom(format!(
                    "spec must define either types or fields. failures: {} ----- {}",
                    Report::new(e1),
                    Report::new(e2)
                )));
            }
        };
        Ok(res)
    }
}

impl<'de, U: DeserializeOwned> Deserialize<'de> for RefOrSpec<U> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = serde_yaml::Value::deserialize(deserializer)?;
        #[derive(Debug, Deserialize)]
        struct Ref {
            #[serde(rename = "ref")]
            name: String,
        }
        let name = Ref::deserialize(&v);
        let single = U::deserialize(&v);
        let res = match (name, single) {
            (Ok(name), _) => Self::Ref { name: name.name },
            (_, Ok(single)) => Self::Spec(single),
            (Err(e1), Err(e2)) => {
                return Err(Error::custom(format!(
                    "spec must define either a ref or a spec. failures: {} ----- {}",
                    Report::new(e1),
                    Report::new(e2)
                )));
            }
        };
        Ok(res)
    }
}

use {
    crate::{
        criteria::{CritLiteralOrRegex, CritUpstreamNode},
        egui_adapter::egui_platform::icons::ICON_CLOSE,
        state::State,
        utils::{numcell::NumCell, static_text::StaticText},
    },
    ahash::AHashSet,
    egui::{ComboBox, DragValue, Ui, UiBuilder, Widget},
    isnt::std_1::collections::IsntHashSetExt,
    linearize::{Linearize, LinearizeExt},
    regex::Regex,
    std::rc::Rc,
};

pub enum CcCriterion<T> {
    Not(Box<Self>),
    List(Vec<Self>, bool),
    Exactly(usize, Vec<Self>),
    T(T),
}

impl<T> Default for CcCriterion<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::T(T::default())
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Linearize)]
enum CompoundCritTy {
    Not,
    All,
    Any,
    Exactly,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum CritTy<T> {
    Compound(CompoundCritTy),
    T(T),
}

impl StaticText for CompoundCritTy {
    fn text(&self) -> &'static str {
        match self {
            Self::Not => "Not",
            Self::All => "All",
            Self::Any => "Any",
            Self::Exactly => "Exactly",
        }
    }
}

impl<T> StaticText for CritTy<T>
where
    T: StaticText,
{
    fn text(&self) -> &'static str {
        match self {
            Self::Compound(t) => t.text(),
            Self::T(t) => t.text(),
        }
    }
}

pub trait CritImpl: Default {
    type Type: Copy + Eq + PartialEq + StaticText + Linearize;
    type Target;

    fn ty(&self) -> Self::Type;
    fn from_ty(ty: Self::Type) -> Self;
    #[must_use]
    fn show(&mut self, ui: &mut Ui) -> bool;

    fn to_crit(&self, state: &Rc<State>) -> Option<Rc<dyn CritUpstreamNode<Self::Target>>>;
    fn not(
        state: &State,
        upstream: &Rc<dyn CritUpstreamNode<Self::Target>>,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>>;
    fn list(
        state: &State,
        upstream: &[Rc<dyn CritUpstreamNode<Self::Target>>],
        all: bool,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>>;
    fn exactly(
        state: &State,
        n: usize,
        upstream: &[Rc<dyn CritUpstreamNode<Self::Target>>],
    ) -> Rc<dyn CritUpstreamNode<Self::Target>>;
}

impl<T> CcCriterion<T>
where
    T: CritImpl,
{
    #[must_use]
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut v = self.ty();
                let old = v;
                ComboBox::from_id_salt("ty")
                    .selected_text(v.text())
                    .show_ui(ui, |ui| {
                        for s in CompoundCritTy::variants() {
                            ui.selectable_value(&mut v, CritTy::Compound(s), s.text());
                        }
                        for s in T::Type::variants() {
                            ui.selectable_value(&mut v, CritTy::T(s), s.text());
                        }
                    });
                if old != v {
                    *self = match v {
                        CritTy::Compound(CompoundCritTy::Not) => {
                            CcCriterion::Not(Default::default())
                        }
                        CritTy::Compound(CompoundCritTy::All) => {
                            CcCriterion::List(Default::default(), true)
                        }
                        CritTy::Compound(CompoundCritTy::Any) => {
                            CcCriterion::List(Default::default(), false)
                        }
                        CritTy::Compound(CompoundCritTy::Exactly) => {
                            CcCriterion::Exactly(1, Default::default())
                        }
                        CritTy::T(t) => CcCriterion::T(T::from_ty(t)),
                    };
                    changed = true;
                }
                match self {
                    CcCriterion::Not(n) => changed |= n.show(ui),
                    CcCriterion::List(_, _) => {}
                    CcCriterion::Exactly(n, _) => {
                        changed |= DragValue::new(n).ui(ui).changed();
                    }
                    CcCriterion::T(t) => changed |= t.show(ui),
                }
            });
            match self {
                CcCriterion::Not(_) => {}
                CcCriterion::List(v, _) | CcCriterion::Exactly(_, v) => {
                    ui.indent("compound", |ui| {
                        let mut to_remove = AHashSet::new();
                        for (idx, v) in v.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.button(ICON_CLOSE).clicked() {
                                    changed = true;
                                    to_remove.insert(idx);
                                }
                                ui.scope_builder(UiBuilder::new().id_salt(idx), |ui| {
                                    changed |= v.show(ui);
                                });
                            });
                        }
                        let i = NumCell::new(0);
                        v.retain(|_| to_remove.not_contains(&i.fetch_add(1)));
                        if ui.button("Add").clicked() {
                            v.push(CcCriterion::default());
                            changed = true;
                        }
                    });
                }
                CcCriterion::T(_) => {}
            }
        });
        changed
    }

    fn ty(&self) -> CritTy<T::Type> {
        match self {
            CcCriterion::Not(_) => CritTy::Compound(CompoundCritTy::Not),
            CcCriterion::List(_, true) => CritTy::Compound(CompoundCritTy::All),
            CcCriterion::List(_, false) => CritTy::Compound(CompoundCritTy::Any),
            CcCriterion::Exactly(_, _) => CritTy::Compound(CompoundCritTy::Exactly),
            CcCriterion::T(t) => CritTy::T(t.ty()),
        }
    }

    pub fn to_crit(&self, state: &Rc<State>) -> Option<Rc<dyn CritUpstreamNode<T::Target>>> {
        match self {
            CcCriterion::Not(t) => Some(T::not(state, &t.to_crit(state)?)),
            CcCriterion::List(v, all) => {
                let mut upstream = Vec::with_capacity(v.len());
                for v in v {
                    upstream.push(v.to_crit(state)?);
                }
                Some(T::list(state, &upstream, *all))
            }
            CcCriterion::Exactly(n, v) => {
                let mut upstream = Vec::with_capacity(v.len());
                for v in v {
                    upstream.push(v.to_crit(state)?);
                }
                Some(T::exactly(state, *n, &upstream))
            }
            CcCriterion::T(t) => t.to_crit(state),
        }
    }

    pub fn any(&self, mut any: impl FnMut(&T) -> bool) -> bool {
        self.any_(&mut any)
    }

    fn any_(&self, any: &mut impl FnMut(&T) -> bool) -> bool {
        match self {
            CcCriterion::Not(v) => v.any_(any),
            CcCriterion::List(v, _) => v.iter().any(|v| v.any_(any)),
            CcCriterion::Exactly(_, v) => v.iter().any(|v| v.any_(any)),
            CcCriterion::T(t) => any(t),
        }
    }
}

pub struct CritRegex {
    pub text: String,
    pub regex: Option<Option<Regex>>,
}

impl Default for CritRegex {
    fn default() -> Self {
        Self {
            text: Default::default(),
            regex: Some(Some(Regex::new("").unwrap())),
        }
    }
}

impl CritRegex {
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut is_regex = self.regex.is_some();
        let mut changed = false;
        changed |= ui.text_edit_singleline(&mut self.text).changed();
        changed |= ui.checkbox(&mut is_regex, "Regex").changed();
        if changed {
            self.regex = is_regex.then(|| Regex::new(&self.text).ok());
        }
        if let Some(None) = self.regex {
            ui.label("Error: Invalid regex");
        }
        changed
    }

    pub fn to_crit(&self) -> Option<CritLiteralOrRegex> {
        match &self.regex {
            None => Some(CritLiteralOrRegex::Literal(self.text.clone())),
            Some(v) => Some(CritLiteralOrRegex::Regex(v.clone()?)),
        }
    }
}

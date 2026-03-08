use crate::{
    AppResult, SearchDirection,
    app::{
        Core, NextAction,
        mode::{search::SearchMode, view::ViewMode},
    },
    screen::Screen,
    source::Source,
};

mod search;
mod view;

pub(super) enum Mode<'app, S, R, Src> {
    View(ViewMode<'app, S, R, Src>),
    Search(SearchMode<'app, S, R, Src>),
}

impl<'app, S: Screen, R, Src: Source<R>> Mode<'app, S, R, Src> {
    pub fn view(core: Core<'app, S, R, Src>) -> Self {
        Self::View(ViewMode::new(core))
    }

    pub fn search(core: Core<'app, S, R, Src>, direction: SearchDirection) -> Self {
        Self::Search(SearchMode::new(core, direction))
    }

    pub fn run(self) -> AppResult<NextAction<'app, S, R, Src>> {
        match self {
            Mode::View(view) => view.run(),
            Mode::Search(search) => search.run(),
        }
    }
}

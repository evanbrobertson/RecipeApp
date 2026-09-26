//! The recipe list, as a `QAbstractListModel` for QML's `GridView`.
//!
//! Like the session, the model logic is plain Rust: [`RecipeListRust`] holds rows and a
//! shared [`SessionCore`]. `refresh` fetches on the shared runtime and applies the result
//! back on the Qt thread, ignoring responses from a superseded search.

use std::sync::Arc;

use core::pin::Pin;

use crumb_client::Error;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};
use tokio::sync::Mutex;

use crate::session::{SessionCore, app_core};

/// One row's data, already formatted for display.
struct Row {
    recipe_id: i64,
    title: String,
    kicker: String,
    image_url: String,
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        #[qobject]
        type QAbstractListModel;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
    }

    #[qenum(RecipeList)]
    enum Roles {
        RecipeId,
        Title,
        Kicker,
        ImageUrl,
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(bool, loading)]
        #[qproperty(i32, count)]
        type RecipeList = super::RecipeListRust;

        /// Emitted when a later call was rejected with 401.
        #[qsignal]
        fn unauthorized(self: Pin<&mut RecipeList>);

        #[qinvokable]
        fn refresh(self: Pin<&mut RecipeList>, query: QString);
    }

    impl cxx_qt::Threading for RecipeList {}

    extern "RustQt" {
        #[qinvokable]
        #[cxx_override]
        fn data(self: &RecipeList, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &RecipeList) -> QHash_i32_QByteArray;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &RecipeList, _parent: &QModelIndex) -> i32;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut RecipeList>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut RecipeList>);
    }
}

/// The inner Rust struct behind the `RecipeList` QObject.
pub struct RecipeListRust {
    core: Arc<Mutex<SessionCore>>,
    rows: Vec<Row>,
    loading: bool,
    count: i32,
    /// Bumped on each `refresh`; an older response is dropped.
    generation: u64,
}

impl Default for RecipeListRust {
    fn default() -> Self {
        Self {
            core: app_core(),
            rows: Vec::new(),
            loading: false,
            count: 0,
            generation: 0,
        }
    }
}

impl qobject::RecipeList {
    pub fn refresh(mut self: Pin<&mut Self>, query: QString) {
        let query = query.to_string();
        self.as_mut().set_loading(true);
        let generation = self.generation.wrapping_add(1);
        self.as_mut().rust_mut().generation = generation;

        let core = self.core.clone();
        let qt_thread = self.as_mut().qt_thread();
        drop(crate::runtime::spawn(async move {
            let client = { core.lock().await.client() };
            let result = match client {
                Some(client) => client
                    .recipes(Some(&query), Some(200))
                    .await
                    .map(|recipes| {
                        recipes
                            .into_iter()
                            .map(|recipe| {
                                let image_url = recipe
                                    .image
                                    .as_deref()
                                    .filter(|image| !image.is_empty())
                                    .map(|image| client.image_url(recipe.id, 320, image))
                                    .unwrap_or_default();
                                let kicker = crumb_core::format::kicker(
                                    recipe.recipe_category.as_deref(),
                                    recipe.recipe_cuisine.as_deref(),
                                );
                                Row {
                                    recipe_id: recipe.id,
                                    title: recipe.title,
                                    kicker,
                                    image_url,
                                }
                            })
                            .collect::<Vec<_>>()
                    }),
                None => Ok(Vec::new()),
            };

            let _ = qt_thread.queue(move |mut object| {
                if object.generation != generation {
                    return;
                }
                match result {
                    Ok(rows) => {
                        let count = rows.len() as i32;
                        unsafe {
                            object.as_mut().begin_reset_model();
                            object.as_mut().rust_mut().rows = rows;
                            object.as_mut().end_reset_model();
                        }
                        object.as_mut().set_count(count);
                    }
                    Err(Error::Unauthorized) => object.as_mut().unauthorized(),
                    Err(_) => {}
                }
                object.as_mut().set_loading(false);
            });
        }));
    }
}

impl qobject::RecipeList {
    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.rows.get(index.row() as usize) else {
            return QVariant::default();
        };
        let roles = qobject::Roles { repr: role };
        match roles {
            qobject::Roles::RecipeId => QVariant::from(&row.recipe_id),
            qobject::Roles::Title => QVariant::from(&QString::from(&row.title)),
            qobject::Roles::Kicker => QVariant::from(&QString::from(&row.kicker)),
            qobject::Roles::ImageUrl => QVariant::from(&QString::from(&row.image_url)),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(qobject::Roles::RecipeId.repr, QByteArray::from("recipeId"));
        roles.insert(qobject::Roles::Title.repr, QByteArray::from("title"));
        roles.insert(qobject::Roles::Kicker.repr, QByteArray::from("kicker"));
        roles.insert(qobject::Roles::ImageUrl.repr, QByteArray::from("imageUrl"));
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rows.len() as i32
    }
}

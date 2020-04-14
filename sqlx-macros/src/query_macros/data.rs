use sqlx::connection::{Connect, Connection};
use sqlx::describe::Describe;
use sqlx::executor::{Executor, RefExecutor};
use url::Url;

use crate::database::DatabaseExt;

#[cfg_attr(feature = "offline", derive(serde::Deserialize, serde::Serialize))]
pub struct QueryData {
    pub(super) input_types: Vec<Option<String>>,
    pub(super) outputs: Vec<(String, String)>,
}

impl QueryData {
    pub fn from_db(db_url: &str, query: &str) -> crate::Result<Self> {
        crate::runtime::block_on(async {
            let db_url = db_url.parse::<Url>()?;

            match db_url.scheme() {
                #[cfg(feature = "sqlite")]
                "sqlite" => {
                    let mut conn = sqlx::sqlite::SqliteConnection::connect(db_url.as_str())
                        .await
                        .map_err(|e| format!("failed to connect to database: {}", e))?;

                    describe_query(conn, query).await
                }
                #[cfg(not(feature = "sqlite"))]
                "sqlite" => Err(format!(
                    "database URL {} has the scheme of a SQLite database but the `sqlite` \
                     feature of sqlx was not enabled",
                    db_url
                )
                .into()),
                #[cfg(feature = "postgres")]
                "postgresql" | "postgres" => {
                    let mut conn = sqlx::postgres::PgConnection::connect(db_url.as_str())
                        .await
                        .map_err(|e| format!("failed to connect to database: {}", e))?;

                    describe_query(conn, query).await
                }
                #[cfg(not(feature = "postgres"))]
                "postgresql" | "postgres" => Err(format!(
                    "database URL {} has the scheme of a Postgres database but the `postgres` \
                     feature of sqlx was not enabled",
                    db_url
                )
                .into()),
                #[cfg(feature = "mysql")]
                "mysql" | "mariadb" => {
                    let mut conn = sqlx::mysql::MySqlConnection::connect(db_url.as_str())
                        .await
                        .map_err(|e| format!("failed to connect to database: {}", e))?;

                    describe_query(conn, query).await
                }
                #[cfg(not(feature = "mysql"))]
                "mysql" | "mariadb" => Err(format!(
                    "database URL {} has the scheme of a MySQL/MariaDB database but the `mysql` \
                     feature of sqlx was not enabled",
                    db_url
                )
                .into()),
                scheme => {
                    Err(format!("unexpected scheme {:?} in database URL {}", scheme, db_url).into())
                }
            }
        })
    }

    pub fn from_file(path: &str, query: &str) -> crate::Result<QueryData> {

    }
}

async fn describe_query<C: Connection>(mut conn: C, query: &str) -> sqlx::Result<QueryData>
where
    <C as Executor>::Database: DatabaseExt,
{
    let describe: Describe<<C as Executor>::Database> = conn.describe(query).await?;

    let input_types = describe.param_types.iter().map(|param_ty| {
        Some(
            DB::param_type_for_id(&param_ty)?
                .parse::<proc_macro2::TokenStream>()
                .unwrap()
        )

    })
}

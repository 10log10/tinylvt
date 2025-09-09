pub mod home;
pub mod logged_in_home;
pub mod logged_out_home;
pub mod login;
pub mod not_found;
pub mod test;

pub use home::HomePage;
pub use logged_in_home::LoggedInHomePage;
pub use logged_out_home::LoggedOutHomePage;
pub use login::LoginPage;
pub use not_found::NotFoundPage;
pub use test::TestPage;

use routetype_hyper::*;
use rust_embed::RustEmbed;
use std::sync::atomic::AtomicUsize;
use tokio::try_join;

#[derive(Route, Clone, PartialEq, Debug)]
enum MyRoute {
    #[route("/")]
    Home,
    #[route("css/style.css")]
    Style,
    #[route("/hello/{name}")]
    Hello { name: String },
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    counter: usize,
    style_css: String,
    greetings: Vec<Greet>,
}

struct Greet {
    name: String,
    route: String,
}

impl From<&str> for Greet {
    fn from(name: &str) -> Self {
        Greet {
            name: name.to_owned(),
            route: MyRoute::Hello {
                name: name.to_owned(),
            }
            .render(),
        }
    }
}

async fn get_home(input: DispatchInput<MyApp>) -> Result<Response<Body>> {
    let counter = input
        .app
        .counter
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let style_css = MyRoute::Style.render(); // FIXME use a OnceCell
    let greetings = vec![
        "Alice".into(),
        "Bob".into(),
        "Charlie".into(),
        "<super dangerous>".into(),
    ];
    respond::askama(HomeTemplate {
        counter,
        style_css,
        greetings,
    })
}

#[derive(Default)]
struct MyApp {
    counter: AtomicUsize,
}

#[async_trait]
impl Dispatch for MyApp {
    type Route = MyRoute;

    async fn dispatch(input: DispatchInput<Self>, route: Self::Route) -> Result<Response<Body>> {
        match route {
            MyRoute::Home => DispatchOutput::into_response(get_home(input).await),
            MyRoute::Style => DispatchOutput::into_response(get_style(input).await),
            MyRoute::Hello { name } => DispatchOutput::into_response(get_hello(input, name).await),
        }
    }
}

async fn get_style(_input: DispatchInput<MyApp>) -> Response<Body> {
    respond::css("h1 { color: red }")
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate {
    name: String,
    home: String,
    style_css: String,
}

async fn get_hello(_input: DispatchInput<MyApp>, name: String) -> Result<Response<Body>> {
    respond::askama(HelloTemplate {
        name,
        home: MyRoute::Home.render(),
        style_css: MyRoute::Style.render(),
    })
}

#[derive(RustEmbed)]
#[folder = "assets"]
struct Assets;

#[tokio::main]
async fn main() -> Result<()> {
    let server = MyApp::default().into_server();

    let plain = server.clone().run(([0, 0, 0, 0], 3000));

    let key = Assets::get("localhost.key").context("localhost.key not found")?;
    let crt = Assets::get("localhost.crt").context("localhost.crt not found")?;
    let config = routetype_hyper::tls::TlsConfigBuilder::new()
        .key(&key)
        .cert(&crt);
    let tls = server.run_tls(([0, 0, 0, 0], 3443), config);

    try_join!(plain, tls)?;

    Ok(())
}

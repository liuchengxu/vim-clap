#[derive(maple_derive::ClapPlugin)]
#[clap_plugin(id = "plugin", actions = ["action7", "action8"])]
struct TestPlugin;

#[derive(maple_derive::ClapPlugin)]
#[clap_plugin(id = "empty")]
struct EmptyPlugin;

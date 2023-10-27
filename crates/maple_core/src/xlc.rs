#[derive(maple_derive::ClapPlugin)]
#[clap_plugin(id = "plugin")]
#[action("action1")]
#[action("action2")]
#[actions("action3", "action4", "__internal_action")]
struct TestPlugin;

#[derive(maple_derive::ClapPlugin)]
#[clap_plugin(id = "empty")]
struct EmptyPlugin;

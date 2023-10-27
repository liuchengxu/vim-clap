#[derive(maple_derive::ClapPlugin)]
#[action("plugin/action1")]
#[action("plugin/action2")]
#[actions("plugin/action3", "plugin/action4", "__internal_action")]
struct TestPlugin;

#[derive(maple_derive::ClapPlugin)]
struct EmptyPlugin;

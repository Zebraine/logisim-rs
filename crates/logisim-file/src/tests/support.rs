pub(super) fn xml(body: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="w" desc="#Wiring"/><lib name="g" desc="#Gates"/><lib name="b" desc="#Base"/><main name="main"/><circuit name="main">{body}</circuit></project>"##
    )
}

pub(super) fn appearance_project(art: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="0" desc="#Wiring"/><main name="top"/>
    <circuit name="top"><comp name="part" loc="(300,200)"><a name="facing" val="east"/></comp></circuit>
    <circuit name="part"><appear>{art}
      <circ-port height="8" pin="100,100" width="8" x="46" y="56"/>
      <circ-port height="10" pin="250,100" width="10" x="75" y="55"/>
      <circ-anchor facing="north" height="6" width="6" x="97" y="67"/>
    </appear><comp lib="0" name="Pin" loc="(100,100)"><a name="width" val="4"/><a name="tristate" val="false"/></comp>
    <comp lib="0" name="Pin" loc="(250,100)"><a name="width" val="4"/><a name="output" val="true"/><a name="facing" val="west"/></comp></circuit></project>"##
    )
}

"""Write the Stage 15 desktop wire-editing fixture to stdout (legacy circuit XML)."""
import argparse
import sys
import xml.etree.ElementTree as ET


def segment(circuit, a, b):
    ET.SubElement(circuit, "wire", {"from": f"({a[0]},{a[1]})", "to": f"({b[0]},{b[1]})"})


def fixture(closed):
    project = ET.Element("project", source="2.16.2.2.jar", version="1.0")
    ET.SubElement(project, "lib", desc="#Wiring", name="0")
    ET.SubElement(project, "lib", desc="#Gates", name="1")
    ET.SubElement(project, "main", name="Wire editing")
    circuit = ET.SubElement(project, "circuit", name="Wire editing")
    # Clamping a NOR from the clock forces a known output before the oscillating half-cycle.
    clock = (80, 160)
    for a, b in [
        ((180, 140), (140, 140)), ((140, 140), (140, 40)), ((140, 40), (190, 40)),
        ((220, 150), (300, 150)), (clock, (180, 160)), ((90, 270), (150, 270)),
    ]:
        segment(circuit, a, b)
    x, y = 300, 150
    for _ in range(11):
        segment(circuit, (x, y), (x + 10, y))
        segment(circuit, (x + 10, y), (x + 10, y - 10))
        x, y = x + 10, y - 10
    segment(circuit, (x, y), (300, 40))
    if closed:
        segment(circuit, (190, 40), (300, 40))
    for i in range(4000):
        x, y = 600 + i % 32 * 50, 800 + i // 32 * 40
        segment(circuit, (x, y), (x + 20, y))
    gate = ET.SubElement(circuit, "comp", lib="1", loc="(220,150)", name="NOR Gate")
    ET.SubElement(gate, "a", name="size", val="30")
    ET.SubElement(gate, "a", name="inputs", val="2")
    ET.SubElement(circuit, "comp", lib="0", loc="(80,160)", name="Clock")
    return ET.ElementTree(project)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--closed", action="store_true", help="include the final feedback wire")
    tree = fixture(parser.parse_args().closed)
    ET.indent(tree)
    tree.write(sys.stdout.buffer, encoding="utf-8", xml_declaration=True)

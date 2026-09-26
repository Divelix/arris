#!/usr/bin/env python3
"""occt_assembly.py <fixture-a> <fixture-b> <out.step>  — build both
fixtures' results in Open CASCADE, make an XCAF assembly of the first
placed once and the second placed twice, write it with
`STEPCAFControl_Writer` — product structure, `NEXT_ASSEMBLY_USAGE_OCCURRENCE`
and `CONTEXT_DEPENDENT_SHAPE_REPRESENTATION` — and print, as JSON, each
placed instance's volume and centroid measured on the placed shape: what
Arris's reader of the file is held to (ADR-0025 §5). Exits 2 with `occt_assembly: ERROR <why>` on stderr when a
recipe does not build or the file is not written. Run as
`uv run --project tools/oracle tools/oracle/occt_assembly.py`.
"""

import json
import math
import sys
from pathlib import Path

from oracle import OracleError, require_ocp

require_ocp()

from OCP.BRepGProp import BRepGProp  # noqa: E402
from OCP.GProp import GProp_GProps  # noqa: E402
from OCP.IFSelect import IFSelect_RetDone  # noqa: E402
from OCP.STEPCAFControl import STEPCAFControl_Writer  # noqa: E402
from OCP.STEPControl import STEPControl_AsIs  # noqa: E402
from OCP.TCollection import TCollection_ExtendedString  # noqa: E402
from OCP.TDocStd import TDocStd_Document  # noqa: E402
from OCP.TopLoc import TopLoc_Location  # noqa: E402
from OCP.XCAFApp import XCAFApp_Application  # noqa: E402
from OCP.XCAFDoc import XCAFDoc_DocumentTool  # noqa: E402
from OCP.gp import gp_Ax1, gp_Dir, gp_Pnt, gp_Trsf, gp_Vec  # noqa: E402

from oracle import step  # noqa: E402
from oracle.fixture import load_fixture  # noqa: E402
from oracle.recipe import build  # noqa: E402


def placement(axis: tuple[float, float, float], degrees: float, by: tuple[float, float, float]) -> gp_Trsf:
    """A turn about `axis` through the origin, then a translation."""
    turn = gp_Trsf()
    turn.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(*axis)), math.radians(degrees))
    shift = gp_Trsf()
    shift.SetTranslation(gp_Vec(*by))
    return shift.Multiplied(turn)


# The first fixture once, the second twice: a turn and a shift each, none
# the identity, so a placement read the wrong way round moves a centroid.
PLACEMENTS = [
    (0, placement((0, 0, 1), 30.0, (10.0, 0.0, 0.0))),
    (1, placement((1, 0, 0), 90.0, (0.0, 20.0, 0.0))),
    (1, placement((0, 1, 1), -45.0, (-7.0, -20.0, 5.0))),
]


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__)
        return 2
    try:
        shapes = [build(load_fixture(Path(d)), "default")[0] for d in argv[:2]]
        app = XCAFApp_Application.GetApplication_s()
        doc = TDocStd_Document(TCollection_ExtendedString("MDTV-XCAF"))
        app.InitDocument(doc)
        tool = XCAFDoc_DocumentTool.ShapeTool_s(doc.Main())
        parts = [tool.AddShape(s, False) for s in shapes]
        assembly = tool.NewShape()
        instances = []
        for which, trsf in PLACEMENTS:
            tool.AddComponent(assembly, parts[which], TopLoc_Location(trsf))
            placed = shapes[which].Moved(TopLoc_Location(trsf))
            props = GProp_GProps()
            BRepGProp.VolumeProperties_s(placed, props)
            c = props.CentreOfMass()
            instances.append({"volume": props.Mass(), "centroid": [c.X(), c.Y(), c.Z()]})
        tool.UpdateAssemblies()
        step.quiet()
        writer = STEPCAFControl_Writer()
        if not writer.Transfer(doc, STEPControl_AsIs):
            raise OracleError("the XCAF transfer failed")
        out = Path(argv[2])
        out.parent.mkdir(parents=True, exist_ok=True)
        if writer.Write(str(out)) != IFSelect_RetDone:
            raise OracleError(f"STEP write failed for {out}")
    except Exception as e:  # OracleError, or Open CASCADE's own
        message = " ".join(str(e).split()) or type(e).__name__
        print(f"occt_assembly: ERROR {message}", file=sys.stderr)
        return 2
    print(json.dumps(instances))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

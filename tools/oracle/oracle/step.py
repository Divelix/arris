"""STEP AP214 in and out through Open CASCADE."""

from pathlib import Path

from OCP.BRepBuilderAPI import BRepBuilderAPI_NurbsConvert
from OCP.IFSelect import IFSelect_RetDone
from OCP.Interface import Interface_Static
from OCP.Message import Message, Message_Gravity
from OCP.STEPControl import STEPControl_AsIs, STEPControl_Reader, STEPControl_Writer
from OCP.TopoDS import TopoDS_Shape

from . import OracleError


def quiet() -> None:
    """Keep only failures and alarms from OCCT's messenger: the STEP transfer
    otherwise prints a statistics banner per file."""
    for printer in list(Message.DefaultMessenger_s().Printers()):
        printer.SetTraceLevel(Message_Gravity.Message_Fail)


def write(shape: TopoDS_Shape, path: Path) -> None:
    quiet()
    Interface_Static.SetCVal_s("write.step.schema", "AP214")
    w = STEPControl_Writer()
    if w.Transfer(shape, STEPControl_AsIs) != IFSelect_RetDone:
        raise OracleError(f"STEP transfer failed for {path}")
    if w.Write(str(path)) != IFSelect_RetDone:
        raise OracleError(f"STEP write failed for {path}")


def read(path: Path) -> TopoDS_Shape:
    if not path.exists():
        raise OracleError(f"no such STEP file: {path}")
    quiet()
    r = STEPControl_Reader()
    if r.ReadFile(str(path)) != IFSelect_RetDone:
        raise OracleError(f"STEP read failed for {path}")
    r.TransferRoots()
    if r.NbShapes() == 0:
        raise OracleError(f"STEP file has no shapes: {path}")
    return r.OneShape()


def nurbs(shape: TopoDS_Shape) -> TopoDS_Shape:
    """`shape` with every surface and curve converted to B-splines by
    `BRepBuilderAPI_NurbsConvert`: the free-form faces with seams and poles
    the STEP reader is held to (plans/step-reader step 15)."""
    converter = BRepBuilderAPI_NurbsConvert(shape, True)
    if not converter.IsDone():
        raise OracleError("BRepBuilderAPI_NurbsConvert failed")
    return converter.Shape()

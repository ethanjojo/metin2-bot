"""Native numeric steps with the same ranges used by profile validation."""
from tkinter import ttk
from decimal import Decimal
from .config import NumberRule


def number_input(parent,*,variable,rule:NumberRule,on_change,width:int=7)->ttk.Spinbox:
    def stepped():
        value=variable.get()
        if '.' in value:variable.set(value.rstrip('0').rstrip('.'))
        on_change()
    entry=ttk.Spinbox(parent,textvariable=variable,wrap=False,width=width,command=stepped)
    set_number_rule(entry,rule)
    entry.bind('<KeyRelease>',lambda event:on_change())
    return entry


def set_number_rule(entry:ttk.Spinbox,rule:NumberRule)->None:
    # Native stepping otherwise rounds to the increment's precision, losing .0001 offsets.
    precision=max(0,*( -Decimal(str(value)).as_tuple().exponent for value in (rule.low,rule.high,rule.step)))
    entry.configure(from_=rule.low,to=rule.high,increment=rule.step,format=f'%.{precision}f')

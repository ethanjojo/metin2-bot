// Quote one Windows argv value, including trailing backslashes and embedded quotes.
function QuoteWindowsArgument(Value: String): String;
var
  I, N, Slashes, Copies: Integer;
begin
  Result := '"';
  Slashes := 0;
  for I := 1 to Length(Value) do
  begin
    if Value[I] = '\' then
      Slashes := Slashes + 1
    else
    begin
      Copies := Slashes;
      if Value[I] = '"' then
        Copies := Slashes * 2 + 1;
      for N := 1 to Copies do
        Result := Result + '\';
      Result := Result + Value[I];
      Slashes := 0;
    end;
  end;
  for N := 1 to Slashes * 2 do
    Result := Result + '\';
  Result := Result + '"';
end;

function UpdateRestartRequested(): Boolean;
var
  DataDir: String;
begin
  DataDir := ExpandConstant('{param:COMPANIONDATADIR|}');
  Result := (ExpandConstant('{param:RESTARTCOMPANION|0}') = '1') and
    not WizardSilent and (DataDir <> '') and (Pos('"', DataDir) = 0) and DirExists(DataDir);
end;

function UpdateRestartArguments(Param: String): String;
begin
  Result := '--data-dir ' + QuoteWindowsArgument(ExpandConstant('{param:COMPANIONDATADIR|}'));
end;

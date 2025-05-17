xcopy /E /Y . ..\7zip\tmp\build\lib7z\
del ..\7zip\tmp\build\lib7z\build.bat
cd ..\7zip\tmp\build\lib7z
nmake /f makefile
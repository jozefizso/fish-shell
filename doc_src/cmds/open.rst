open - open file in its default application
===========================================

Synopsis
--------

.. synopsis::

    open FILES ...


Description
-----------

``open`` opens a file in its default application, using the appropriate tool for the operating system. On GNU/Linux, this requires the common but optional ``xdg-open`` utility, from the ``xdg-utils`` package.

On native Windows, ``open`` uses the system's native file/URL associations, passing each item literally without ``cmd /c start`` or a shell command string. A missing association or failed open returns a nonzero status.

On other systems this function will not be used if a command by this name exists (which is the case on macOS or Haiku).


Example
-------

``open *.txt`` opens all the text files in the current directory using your system's default text editor.

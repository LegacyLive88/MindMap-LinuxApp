# MindMap-LinuxApp

An application that works almost like Obsidian Canvas, except for a few differences:
1. It is essentially a mindmapping tool whereby you can start a new canvas, each of which has one circle in the middle. Holding in Ctrl and clicking on either the middle circle or a text rectangle connected to it adds a line (straight or wavy that avoids crossing rectangles or the circle, and where possible avoids crossing other lines) and a new rectangle. Holding in Ctrl and clicking and dragging between rectangle connects rectangles to each other. Clicking on a rectangle (without Ctrl) allows one to select delete or close/open it (a closed rectangle element and its children is closed, greyed out, and can't be edited; deleting a rectangle deletes it and all lines connected to it) and also shows when it was created and when last it or any of its children (recursive across children's children...) was last added to or edited (collectively referred to as modified date). Double clicking on a rectangle (without Ctrl) starts editing its text if it's not closed. Deleting or adding rectangles or connecting lines auto-arranges all the elements affected for optimal spacing efficiency and clarity. 
- Clicking shift and clicking an element opens a new canvas with that element's wording in the centre circle (there can be infinite dimensions like this) with a back button in the UI to go back to the parent canvas focused on that rectangle again.
- Arrow keys allows the use to scroll around on the canvas, space bar (when not editing an element scrolls until the circle element is in the middle of the screen, '+' and '-' keys allow the user to zoom in and zoom out, and '=' key zooms to fit
- When a circle hasn't been modified for a week its colour becomes yellow, for a month orange, and for 6 months red.
- Changes should be saved as they happen
- There should be a single place where one can view all yellow, orange and red elements. 

The app should be a portable file with an unencrypted folder (in the same folder as the portable file) containing all data for all canvases. The app should run on Linux Cinnamon Mint can be built in any common language but should be able to be easily compiled into a single file app.


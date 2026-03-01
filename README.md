# rcrosshair
Display custom images or GIFs as crosshair overlay on wlroots compositors

## Usage
```bash
~ rcrosshair help
Usage: rcrosshair [OPTIONS] <IMAGE_PATH> [COMMAND]

Commands:
  clear  Clear cached parameters for an image
  help   Print this message or the help of the given subcommand(s)

Arguments:
  <IMAGE_PATH>  

Options:
  -x, --target-x <TARGET_X>  The x coordinate on the image to be centered
  -y, --target-y <TARGET_Y>  The y coordinate on the image to be centered
  -o, --opacity <OPACITY>    range from 0 to 1
  -h, --help                 Print help
```

### Examples
```bash
# Center the center of the image and show with full opacity by default
rcrosshair images/example.png

# Center the crosshair on specific coordinates
rcrosshair images/example.png -x 192 -y 42
# Center horizontally at x=192, but keep y at the image center
rcrosshair images/example.png -x 192
# Center horizontally at y=42, but keep x at the image center
rcrosshair images/example.png -y 42

# Make the crosshair semi-transparent
rcrosshair images/example.png -o 0.5
# Even more transparency
rcrosshair images/example.png -o 0.3

# Combined
rcrosshair images/example.png -x 192 -y 42 -o 0.5

# The same goes for GIFs
rcrosshair images/example.gif
rcrosshair images/example.gif -x 192 -y 42 -o 0.5

# Clear cached parameters for an image
rcrosshair images/example.png clear
```

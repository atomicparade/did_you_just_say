# did_you_just_say



Discord bot that inserts text into images. Built with [Serenity](https://github.com/serenity-rs/serenity).

# Bot configuration

1. Obtain images and fonts.
2. Copy `.env.EXAMPLE` to `.env`. Put the Discord bot token in this file and set an administration password (used to authenticate so that you can shut down the bot).
3. Copy `config.yml.EXAMPLE` to `config.yml`. Enter the details about each image in this file.

```yml
- filename: "memes/example.png"
  font: "fonts/font.ttf"
  font_size: 12
  left: 100
  top: 100
  right: 300
  bottom: 300
  text_prefix: ""
  text_suffix: ""
  command: "example"
  is_default: true
```

`left`, `top`, `right`, `bottom`: These describe the bounding box of the text. The text will automatically be placed in the center.
`command`: When a user sends `@Bot command some text`, the bot will insert "some text" into the image.
`is_default`: When a user sends `@Bot some text` (without a command), the bot will use this image.
`text_prefix`, `text_suffix`: These will automatically be inserted before/after the text specified by the user.

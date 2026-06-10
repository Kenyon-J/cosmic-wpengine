import re

with open("src/modules/gui/view.rs", "r") as f:
    content = f.read()

content = content.replace(".align_items(cosmic::iced::Alignment::Center)", ".align_y(cosmic::iced::alignment::Vertical::Center)")

with open("src/modules/gui/view.rs", "w") as f:
    f.write(content)

import re

with open("src/modules/gui/view.rs", "r") as f:
    content = f.read()

# I used .align_y(cosmic::iced::Alignment::Center) but the compiler said ok... wait, wait, the framework is using an older iced? Let me test .align_items
# Actually the review says: "By combining .align_y() with Alignment::Center, the code will fail to compile in either version of the framework...". But my `cargo check` passed.
# Let me replace .align_y(cosmic::iced::Alignment::Center) with .align_items(cosmic::iced::Alignment::Center)
content = content.replace(".align_y(cosmic::iced::Alignment::Center)", ".align_items(cosmic::iced::Alignment::Center)")

with open("src/modules/gui/view.rs", "w") as f:
    f.write(content)

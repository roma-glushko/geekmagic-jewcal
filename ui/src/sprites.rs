//! Pixel art. Letters are Sweetie-16 colors (see `palette::from_letter`):
//! k black · p purple · r red · o orange · y yellow · l lime · g green ·
//! t teal · n navy · b blue · s sky · c cyan · w white · h silver ·
//! m slate · d dark · `.` transparent.
//!
//! Flames are separate 5×6 sprites with three frames so every candle on
//! screen (Shabbat, memorial, menorah) flickers the same way.

use crate::sprite;
use crate::sprite::Sprite;

pub const FLAME: [Sprite; 3] = [
    sprite!(5, 6, ["..o..", ".oyo.", ".oyo.", "oywyo", "oywyo", ".oyo."]),
    sprite!(5, 6, [".o...", "..o..", ".oyo.", "oywyo", "oywyo", ".oyo."]),
    sprite!(5, 6, ["...o.", "..o..", ".oyo.", ".oyo.", "oywyo", ".oyo."]),
];

/// Two Shabbat candles; flames go at (4, 1) and (15, 1).
pub const CANDLES: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "......m..........m......",
        ".....www........www.....",
        ".....wwh........wwh.....",
        ".....wwh........wwh.....",
        ".....wwh........wwh.....",
        ".....wwh........wwh.....",
        ".....wwh........wwh.....",
        ".....wwh........wwh.....",
        ".....wwh........wwh.....",
        ".....wwh........wwh.....",
        ".....wwh........wwh.....",
        "....yyyyy......yyyyy....",
        ".....oyo........oyo.....",
        "......o..........o......",
        "......o..........o......",
        "....yyyyy......yyyyy....",
        "...ooooooo....ooooooo...",
    ]
);
pub const CANDLES_FLAMES: [(i32, i32); 2] = [(4, 1), (15, 1)];

/// Memorial candle in a glass (Yom Kippur, Yom HaShoah, Yom HaZikaron);
/// flame at (9, 1).
pub const MEMORIAL: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "...........m............",
        ".....hhhhhhhhhhhhhh.....",
        ".....hyyyyywyyyyyyh.....",
        ".....hwwwwwwwwwwwwh.....",
        "......hwwwwwwwwwwh......",
        "......hwwwwwwwwwhh......",
        "......hwwwwwwwwwwh......",
        "......hhwwwwwwwwwh......",
        ".......hwwwwwwwwh.......",
        ".......hwwwwwwwwh.......",
        ".......hhwwwwwwhh.......",
        ".......hwwwwwwwwh.......",
        "........hwwwwwwh........",
        "........hwwwwwwh........",
        "........hhhhhhhh........",
        ".......mmmmmmmmmm.......",
        "........................",
    ]
);
pub const MEMORIAL_FLAME: (i32, i32) = (9, 1);

/// Praying hands with tallit-striped sleeves (Yom Kippur).
pub const PRAYING_HANDS: Sprite = sprite!(
    24,
    24,
    [
        ".........hhhhh..........",
        "........hwwmwwh.........",
        "........hwwmwwh.........",
        ".......hmwwmwwmh........",
        ".......hmwwmwwmh........",
        "..s...hwmwwmwwmwh...s...",
        "...s..hmwmwmwmwmh..s....",
        "......hmwmwmwmwmh.......",
        ".....hwmwmwmwmwmwh......",
        ".s...hmwmwwmwwmwmh...s..",
        ".....hmwmwhmhwmwmh......",
        ".....hmwwwhmhwwwmh......",
        ".....hmwwwh.hwwwmh......",
        ".....hwmwwh.hwwmwh......",
        "......hwmwh.hwmwh.......",
        ".......hhhh.hhhh........",
        "......bbsbsbbsbsbb......",
        "......ssssss.ssssss.....",
        ".....sssssss.sssssss....",
        ".....bbsbsbb.bbsbsbb....",
        "....ssssssss.ssssssss...",
        "....ssssssss.ssssssss...",
        "...sssssssss.sssssssss..",
        "...bbbbbbbbb.bbbbbbbbb..",
    ]
);

/// The Western Wall: Herodian stones with chiseled margins, notes in the
/// cracks, hyssop growing from the joints (Tisha B'Av).
pub const WALL: Sprite = sprite!(
    30,
    22,
    [
        "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkk",
        "kmmmmmkmmmmmmmmkmmmmmmmkmmmmmm",
        "kmhhhmkmhhhhhhmkmhhhhhmkmhhhhm",
        "kmmmmmkmgmgmmmmkmmmmmmmkmmmmmm",
        "kkkkkkkkkgkkkkkkkkkwkkkkkkkkkk",
        "kmmmmmmmmkmmmmmmkmmmmmmmmkmmmm",
        "kmhhhhhhmkmhhhhmkmhhhhhhmkmhhm",
        "kmmmmmmmmkmmmmmmkmmmmmmmgkmmmm",
        "kkkkkkwkkkkkkkkkkkkkkkkgkkkkkk",
        "kmmmmkmmmmmmmmmkmmmmmmkmmmmmmm",
        "kmhhmkmhhhhhhhmkmhhhhmkmhhhhhm",
        "kmhhmkmhhhhhhhmkmhhhhmkmhhhhhm",
        "kmmmmkmmmmmmmmmkgmmmmmkmmmmmmm",
        "kkkkkkkkkkkkkkkgkkkkkkkkkkwkkk",
        "kmmmmmmmkmmmmmmmkmmmmmmmmkmmmm",
        "kmhhhhhmkmhhhhhmkmhhhhhhmkmhhm",
        "kmhhhghmkmhhhhhmkmhhhhhhmkmhhm",
        "kmmmgmmmkmmmmmmmkmmmmmmmmkmmmm",
        "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkk",
        "kmmmmmmmmmmkmmmmmkmmmmmmkmmmmm",
        "kmhhhhhhhhmkmhhhmkmhhhhmkmhhhm",
        "kmmmmmmmmmmkmmmmmkmmmmmmkmmmmm",
    ]
);

/// Shofar and an apple (Rosh Hashana).
pub const SHOFAR: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "........................",
        "...................ooo..",
        "..................onnno.",
        ".................onnnnno",
        ".................oonnnoo",
        "................oyoooyo.",
        "................oyyyyyo.",
        "...............oyyyyyo..",
        "...............oywyyyo..",
        "..............oywyyyo...",
        ".............oywyyyo....",
        "............oywyyyo.....",
        "..........ooywyyyo......",
        "..ooooooooywyyyyo.......",
        ".oyyyyyyyyyyyyoo........",
        ".oywwyyyyyyyyo..........",
        "..ooyyyyyyooo....gg.....",
        "....oooooo......grrrg...",
        "...............rrrwrrr..",
        "...............rrwrrrr..",
        "...............rrrrrrr..",
        "................rrrrr...",
    ]
);

/// Sukkah: schach roof, posts, hanging fruit.
pub const SUKKAH: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "....g......g.......g....",
        "..g.gg.g..ggg..g..ggg.g.",
        ".gglgggglgglgggglggglgg.",
        "gglggggggglgggggglgggggg",
        "gggtggggtgggggtggggtgggt",
        ".oo..r....y....r....yoo.",
        ".oo..r....y....r....yoo.",
        ".oo.......y.........oo..",
        ".oodmdmdmdmdmdmdmdmdoo..",
        ".oodmdmdmdmdmdmdmdmdoo..",
        ".oodmdmdmdmdmdmdmdmdoo..",
        ".oodmdmdmdmdmdmdmdmdoo..",
        ".oodmdmdmdmdmdmdmdmdoo..",
        ".oodmdmdmdmdmdmdmdmdoo..",
        ".oodmdmyyyyyyyyydmdmoo..",
        ".oodmdmooooooooodmdmoo..",
        ".oodmdmdo.....odmdmdoo..",
        ".oodmdmdo.....odmdmdoo..",
        ".oodmdmdmdmdmdmdmdmdoo..",
        ".oodmdmdmdmdmdmdmdmdoo..",
        "googgggggggggggggggoogg.",
        "lgglgggglggggglgggglggl.",
        "........................",
    ]
);

/// Matzah and a cup of wine (Pesach).
pub const MATZAH: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "...............hhhhhhh..",
        "...............hrrrrrh..",
        "...............hrrrrrh..",
        "...............hrrrrrh..",
        "................hrrrh...",
        ".................hhh....",
        "..................h.....",
        "..oooooooooooo....h.....",
        "..oyyyyyyyyyyo...hhh....",
        "..oyoyoyoyoyyo..hhhhh...",
        "..oyyyyyyyyyyo..........",
        "..oyyoyoyoyoyo..........",
        "..oyyyyyyyyyyo..........",
        "..oyoyoyoyoyyo..........",
        "..oyyyyyyyyyyo..........",
        "..oyyoyoyoyoyo..........",
        "..oyyyyyyyyyyo..........",
        "..oyoyoyoyoyyo..........",
        "..oyyyyyyyyyyo..........",
        "..oooooooooooo..........",
        "........................",
        "........................",
    ]
);

/// Tablets of the covenant (Shavuot).
pub const TABLETS: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "....hhhhh....hhhhh......",
        "...hwwwwwh..hwwwwwh.....",
        "..hwwwwwwwhhwwwwwwwh....",
        "..hwmmmwwwhhwmmmwwwh....",
        "..hwwwwwwwhhwwwwwwwh....",
        "..hwmmmmwwhhwmmmmwwh....",
        "..hwwwwwwwhhwwwwwwwh....",
        "..hwmmmwwwhhwmmmwwwh....",
        "..hwwwwwwwhhwwwwwwwh....",
        "..hwmmmmwwhhwmmmmwwh....",
        "..hwwwwwwwhhwwwwwwwh....",
        "..hwmmmwwwhhwmmmwwwh....",
        "..hwwwwwwwhhwwwwwwwh....",
        "..hwmmmmwwhhwmmmmwwh....",
        "..hwwwwwwwhhwwwwwwwh....",
        "..hhhhhhhhhhhhhhhhhh....",
        "..........g.l...........",
        "........ggglgg..........",
        ".......lgg.gglg.........",
        "........................",
        "........................",
        "........................",
    ]
);

/// Torah scroll (Simchat Torah).
pub const TORAH: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "...yy..............yy...",
        "...oo..............oo...",
        "..oooo............oooo..",
        "..oooowwwwwwwwwwwwoooo..",
        "..oooowmmwwmmmwwwwoooo..",
        "..oooowwwwwwwwwwwwoooo..",
        "..oooowmmmwwmmwwwwoooo..",
        "..oooowwwwwwwwwwwwoooo..",
        "..oooowmmwwwmmmwwwoooo..",
        "..oooowwwwwwwwwwwwoooo..",
        "..oooowmmmmwwmmwwwoooo..",
        "..oooowwwwwwwwwwwwoooo..",
        "..oooowmmwwmmmmwwwoooo..",
        "..oooowwwwwwwwwwwwoooo..",
        "..oooowmmmwwwmmwwwoooo..",
        "..oooowwwwwwwwwwwwoooo..",
        "..oooowwwwwwwwwwwwoooo..",
        "..oooo............oooo..",
        "...oo..............oo...",
        "...yy..............yy...",
        "...yy..............yy...",
        "........................",
        "........................",
    ]
);

/// Crescent moon with stars (Rosh Chodesh).
pub const MOON: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "........yyyy............",
        "......yyyy.....w........",
        ".....yyyy...............",
        "....yyyy...........y....",
        "...yyyy...........yyy...",
        "...yyyy............y....",
        "..yyyy..................",
        "..yyyy.........w........",
        "..yyyy..................",
        "..yyyy..................",
        "..yyyy..................",
        "..yyyyy.................",
        "...yyyy.................",
        "...yyyyy.........w......",
        "....yyyyy...............",
        ".....yyyyyy.............",
        "......yyyyyyy...........",
        "........yyyyyyyy........",
        "..........yyyyyyy.......",
        "........................",
        "........................",
        "........................",
    ]
);

/// Empty plate, fork and knife (fast days).
pub const PLATE: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "........................",
        "........................",
        "..h.h.h............hh...",
        "..h.h.h............hhh..",
        "..h.h.h...hhhhh....hhh..",
        "..hhhhh.hhwwwwwhh..hhh..",
        "...hhh.hwwmmmmmwwh.hhh..",
        "....h..hwmdddddmwh.hhh..",
        "....h.hwmdddddddmwhhhh..",
        "....h.hwmdddddddmwh.h...",
        "....h.hwmdddddddmwh.h...",
        "....h.hwmdddddddmwh.h...",
        "....h..hwmdddddmwh..h...",
        "....h..hwwmmmmmwwh..h...",
        "....h...hhwwwwwhh...h...",
        "....h.....hhhhh.....h...",
        "....h...............h...",
        "...rrr.............rrr..",
        "........................",
        "........................",
        "........................",
        "........................",
    ]
);

/// Israeli flag (modern days).
pub const FLAG: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "........................",
        "........................",
        ".h......................",
        ".hwwwwwwwwwwwwwwwwwwwww.",
        ".hbbbbbbbbbbbbbbbbbbbbb.",
        ".hwwwwwwwwwwwwwwwwwwwww.",
        ".hwwwwwwwwwbwwwwwwwwwww.",
        ".hwwwwwwwwbwbwwwwwwwwww.",
        ".hwwwwwwbbbbbbbwwwwwwww.",
        ".hwwwwwwwbwwwbwwwwwwwww.",
        ".hwwwwwwbbbbbbbwwwwwwww.",
        ".hwwwwwwwwbwbwwwwwwwwww.",
        ".hwwwwwwwwwbwwwwwwwwwww.",
        ".hwwwwwwwwwwwwwwwwwwwww.",
        ".hbbbbbbbbbbbbbbbbbbbbb.",
        ".hwwwwwwwwwwwwwwwwwwwww.",
        ".h......................",
        ".h......................",
        ".h......................",
        ".h......................",
        ".m......................",
        "........................",
    ]
);

/// Purim mask.
pub const MASK: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "...p................p...",
        "...pp..............pp...",
        "...ppp............ppp...",
        "...pppppppppppppppppp...",
        "..pppyyyyppppppyyyyppp..",
        "..ppykkkkyppppykkkkyppp.",
        "..ppykkkkyppppykkkkypp..",
        "..pppyyyyppyyppyyyyppp..",
        "...ppppppppyyppppppppp..",
        "....pppppp....pppppp....",
        ".....pppp......pppp.....",
        "......pp........pp......",
        "...........y............",
        "..........yry...........",
        "...........y............",
        "...........r............",
        "..........r.............",
        "........................",
        "........................",
    ]
);

/// Magen David (any other Yom Tov or holiday).
pub const STAR: Sprite = sprite!(
    24,
    24,
    [
        "........................",
        "........................",
        "...........y............",
        "..........yyy...........",
        ".........yy.yy..........",
        "........yy...yy.........",
        "..yyyyyyyyyyyyyyyyyyy...",
        "...yy...yy...yy...yy....",
        "....yy.yy.....yy.yy.....",
        ".....yyy.......yyy......",
        "......yy.......yy.......",
        ".....yyy.......yyy......",
        "....yy.yy.....yy.yy.....",
        "...yy...yy...yy...yy....",
        "..yyyyyyyyyyyyyyyyyyy...",
        "........yy...yy.........",
        ".........yy.yy..........",
        "..........yyy...........",
        "...........y............",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ]
);

// ── 7-pixel inline icons ─────────────────────────────────────────────────────

/// Candle lit before sunset (a candle with the sun setting behind it).
pub const ICON_CANDLE: Sprite =
    sprite!(7, 8, ["...o...", "..oyo..", "..oyo..", "...m...", "..www..", "..wwh..", "..wwh..", ".yyyyy.",]);

/// Candle lit after nightfall (candle under a star).
pub const ICON_NIGHT: Sprite =
    sprite!(7, 8, ["w....o.", "....oyo", "...m.o.", "..www..", "..wwh..", "..wwh..", "..wwh..", ".yyyyy.",]);

/// Braided havdalah candle.
pub const ICON_HAVDALAH: Sprite =
    sprite!(7, 8, ["..o.o..", ".oyoyo.", "..yoy..", "..bwb..", "..wbw..", "..bwb..", "..wbw..", "...b...",]);

pub const ICON_PLATE: Sprite =
    sprite!(7, 8, ["h.h..h.", "h.h.hh.", "hhh.hh.", ".h..hh.", ".h...h.", ".h...h.", ".h...h.", ".......",]);

pub const ICON_MOON: Sprite =
    sprite!(7, 8, ["..hhh..", ".hh....", "hh.....", "hh.....", "hh.....", ".hh....", "..hhh..", ".......",]);

pub const ICON_STAR: Sprite =
    sprite!(7, 8, ["...y...", "yyyyyyy", ".y.y.y.", "..y.y..", ".y.y.y.", "yyyyyyy", "...y...", ".......",]);

pub const ICON_DOT: Sprite =
    sprite!(7, 8, [".......", ".......", "..lll..", ".lllll.", ".lllll.", "..lll..", ".......", ".......",]);

pub const ICON_FLAG: Sprite =
    sprite!(7, 8, [".......", "wwwwwww", "bbbbbbb", "www.www", "ww.b.ww", "bbbbbbb", "wwwwwww", ".......",]);

pub const ICON_MENORAH: Sprite =
    sprite!(7, 8, ["y.y.y.y", "o.o.o.o", "h.h.h.h", "h.h.h.h", "hhhhhhh", "...h...", "...h...", "..hhh..",]);

pub const ICON_WIFI: Sprite =
    sprite!(9, 7, ["..sssss..", ".s.....s.", "s..sss..s", "..s...s..", "....s....", "...sss...", "....s....",]);

// ── Holiday screens ──────────────────────────────────────────────────────────

/// Lulav bound with myrtle and willow, and an etrog (Sukkot).
pub const LULAV: Sprite = sprite!(
    16,
    24,
    [
        "......l.........",
        "......gl........",
        ".....ggl........",
        ".....ggl........",
        ".....ggl........",
        "....gggl........",
        "....gglg........",
        "...tgglgt.......",
        "..ttgglgtt......",
        "...tgglgt.......",
        "..ttgglgtt......",
        ".l.tgglgt.l.....",
        "..ltgglgtl......",
        "...tgglgt.......",
        "....oooo........",
        "....gglg..ggg...",
        "....gglg.yyyyy..",
        "....gglgyywyyyo.",
        "....gglgyyyyyyo.",
        "....gglgyyyyyyo.",
        "....gglg.yyyyo..",
        "....gglg..ooo...",
        ".....gg.........",
        "................",
    ]
);

/// Sheaf of wheat (the Omer, Shavuot).
pub const WHEAT: Sprite = sprite!(
    12,
    18,
    [
        "..y...y...y.",
        ".yoy.yoy.yoy",
        "..y...y...y.",
        ".yoy.yoy.yoy",
        "..y...y...y.",
        ".yoy.yoy.yoy",
        "..y...y...y.",
        "..y...y..y..",
        "...y..y..y..",
        "...y..y.y...",
        "....y.yy....",
        "....yyyy....",
        "....rrrr....",
        "....yyyy....",
        "...y.yy.y...",
        "..y..yy..y..",
        "..y..yy..y..",
        ".y...yy...y.",
    ]
);

/// Hamantasch (Purim).
pub const HAMANTASCH: Sprite = sprite!(
    14,
    12,
    [
        "......oo......",
        ".....oyyo.....",
        ".....oyyo.....",
        "....oyyyyo....",
        "....oyppyo....",
        "...oyppppyo...",
        "...oyppppyo...",
        "..oyyppppyyo..",
        "..oyyyppyyyo..",
        ".oyyyyyyyyyyo.",
        "oyyyyyyyyyyyyo",
        "oooooooooooooo",
    ]
);

/// Jar of honey (Rosh Hashana).
pub const HONEY: Sprite = sprite!(
    12,
    12,
    [
        "...tttttt...",
        "..tttttttt..",
        "...hhhhhh...",
        "..h......h..",
        ".hyyyyyyyyh.",
        ".hyoyyyyyyh.",
        ".hyyyyyoyyh.",
        ".hyyyoyyyyh.",
        ".hyyyyyyyyh.",
        ".hoyyyyyyoh.",
        "..hyyyyyyh..",
        "...hhhhhh...",
    ]
);

pub const ICON_SHOFAR: Sprite = sprite!(
    11,
    8,
    [
        "........oo.",
        ".......onno",
        ".......oyno",
        "......oyyo.",
        "o....oyyo..",
        "oyooyywo...",
        ".oyyyyo....",
        "..oooo.....",
    ]
);

/// The four mitzvot of Purim: megillah, gifts of food, gifts to the poor, feast.
pub const ICON_SCROLL: Sprite = sprite!(
    11,
    9,
    [
        "oo.......oo",
        "oowwwwwwwoo",
        "oowmmwmmwoo",
        "oowwwwwwwoo",
        "oowmmmwmwoo",
        "oowwwwwwwoo",
        "oowmwmmmwoo",
        "oowwwwwwwoo",
        "oo.......oo",
    ]
);

pub const ICON_GIFT: Sprite = sprite!(
    11,
    9,
    [
        "..rr...rr..",
        "...rr.rr...",
        "bbbbbrbbbbb",
        "bsssbrbsssb",
        "bsssbrbsssb",
        "rrrrrrrrrrr",
        "bsssbrbsssb",
        "bsssbrbsssb",
        "bbbbbrbbbbb",
    ]
);

pub const ICON_COINS: Sprite = sprite!(
    11,
    9,
    [
        "...yyyyy...",
        "..yoooooy..",
        "...yyyyy...",
        ".yyyyy.....",
        "yoooooy....",
        ".yyyyyyyyy.",
        "....yoooooy",
        ".....yyyyy.",
        "...........",
    ]
);

pub const ICON_FEAST: Sprite = sprite!(
    11,
    9,
    [
        "...........",
        ".hhhhh.....",
        ".hrrrh..y..",
        ".hrrrh.yoy.",
        "..hrh.yoooy",
        "...h..yoooy",
        "...h..ooooo",
        "..hhh......",
        "...........",
    ]
);

// ── Weather ──────────────────────────────────────────────────────────────────

/// Sun disc; the rays are drawn around it and pulse.
pub const SUN: Sprite = sprite!(
    11,
    11,
    [
        "...ooooo...",
        "..oyyyyyo..",
        ".oyywyyyyo.",
        "oyywyyyyyyo",
        "oyyyyyyyyyo",
        "oyyyyyyyyyo",
        "oyyyyyyyyyo",
        "oyyyyyyyyyo",
        ".oyyyyyyyo.",
        "..oyyyyyo..",
        "...ooooo...",
    ]
);

pub const MOON_NIGHT: Sprite = sprite!(
    10,
    12,
    [
        "...yyyy...",
        ".yyyyy....",
        ".yyyy.....",
        "yyyy......",
        "yyyy......",
        "yyyy......",
        "yyyy......",
        "yyyy......",
        "yyyyy.....",
        ".yyyyy....",
        "..yyyyyy..",
        "....yyyy..",
    ]
);

pub const CLOUD: Sprite = sprite!(
    22,
    10,
    [
        "..........wwwww.......",
        "........wwwwwwwww.....",
        "....www.wwwwwwwwwh....",
        "..wwwwwwwwwwwwwwwwh...",
        ".wwwwwwwwwwwwwwwwwwhh.",
        "wwwwwwwwwwwwwwwwwwwwhh",
        "wwwwwwwwwwwwwwwwwwwwhh",
        "hwwwwwwwwwwwwwwwwwwwhh",
        ".hhwwwwwwwwwwwwwwwwhh.",
        "...hhhhhhhhhhhhhhhh...",
    ]
);

pub const SUN_SMALL: Sprite =
    sprite!(7, 7, ["...y...", ".y.y.y.", "..yyy..", "yyyyyyy", "..yyy..", ".y.y.y.", "...y...",]);

pub const MOON_SMALL: Sprite = sprite!(6, 7, ["..yyy.", ".yy...", "yy....", "yy....", "yy....", ".yy...", "..yyy.",]);

pub const CLOUD_SMALL: Sprite =
    sprite!(10, 6, ["....www...", "..wwwwwh..", ".wwwwwwwh.", "wwwwwwwwwh", "hwwwwwwwhh", ".hhhhhhhh.",]);

pub const ICON_WIND: Sprite =
    sprite!(7, 7, ["....hh.", ".....h.", "hhhhhh.", ".......", "hhhhhhh", "......h", "....hh.",]);

pub const ICON_HUMIDITY: Sprite =
    sprite!(7, 7, ["...s...", "..sss..", ".sssss.", ".sscss.", "sssssss", ".sssss.", "..sss..",]);

pub const ICON_PRESSURE: Sprite =
    sprite!(7, 7, [".hhhhh.", "h..w..h", "h..w..h", "h..y..h", "h.....h", ".hhhhh.", ".......",]);

pub const ICON_UMBRELLA: Sprite =
    sprite!(7, 7, ["...r...", ".rrrrr.", "rrrrrrr", "...h...", "...h...", "...h.h.", "....h..",]);

// Editorial changes are separate from general Unicode/markup normalization.
// Each change requires the exact previous value; unexpected input is an error.
pub struct Correction {
    pub id: u32,
    pub field: &'static str,
    pub before: &'static str,
    pub after: &'static str,
    pub reason: &'static str,
}

pub const HTML: &[Correction] = &[
    Correction {
        id: 83,
        field: "raw_html",
        before: "<div>&nbsp;&nbsp;\n\t\t\t<lang>A</lang> <hw><pa>ابجد</pa> <i>abjad</i></hw> , Arrangement of the Arabic alphabet, according to the numeral value of the letters from one to a thousand, as follows: <p><img src=\"/dictionaries/steingass/images/abjad_p0005.jpg\" alt=\"Arabic alphabet according to numeral values\" width=\"50%\" border=\"0\"></p><p> <i>abjadi tajrīd nawishtan</i>, To abandon the world, to devote one's self wholly to God; — <i>abjad-ḵẖẉān</i>, One learning his alphabet. \n\t\t\t</p></div>",
        after: "<div> <lang>A</lang> <hw><pa>ابجد</pa> <i>abjad</i></hw> , Arrangement of the Arabic alphabet, according to the numeral value of the letters from one to a thousand, as follows: <br> <img src=\"/dictionaries/steingass/images/abjad_p0005.jpg\" alt=\"Arabic alphabet according to numeral values\" border=\"0\" width=\"50%\"> <br> <i>abjadi tajrīd nawishtan</i>, To abandon the world, to devote one's self wholly to God; — <i>abjad-ḵẖẉān</i>, One learning his alphabet. </div>",
        reason: "Repair abjad image markup; historical commit 78996b37c32f.",
    },
    Correction {
        id: 479,
        field: "raw_html",
        before: "<div>&nbsp;&nbsp;\n\t\t\t<hw><pa>آتش آلود</pa> <i>ātish-ālūd</i>, <pa>آتش اندود</pa> <i>ātish andūd. &gt; Suffused with fire. \n\t\t\t</i></hw></div>",
        after: "<div><hw><pa>آتش آلود</pa> <i>ātish-ālūd</i>, <pa>آتش اندود</pa> <i>ātish andūd</i></hw> , Suffused with fire.</div>",
        reason: "Repair misplaced headword/italic boundaries; commits 5c1d22c2d4a5 and a5bc7c0e164b.",
    },
    Correction {
        id: 5267,
        field: "raw_html",
        before: "<div>&nbsp;&nbsp;\n\t\t\t<hw><pa>اندروصاخس</pa> <i>andarūmāchus</i></hw> (G. <at>)*andro/maxos</at>), Andromachus. \n\t\t\t</div>",
        after: "<div><hw><pa>اندروماخس</pa> <i>andarūmāchus</i></hw> (G. <at>Ἀνδρόμαχος</at>), Andromachus.</div>",
        reason: "Correct Andromachus in Persian and Greek; commits 720d273b6632 and 5831830b582d.",
    },
    Correction {
        id: 63287,
        field: "raw_html",
        before: "<div>&nbsp;&nbsp;\n\t\t\t<lang>A</lang> <hw><pa>نجز</pa> (v.n.), Performing (business), fulfilling (a promise); being present, in readiness; — <i>najaz</i></hw> (v.n.) Being performed (a promise), being finished (a business or discourse); being at the end of a journey; perishing, being ruined; vanishing. \n\t\t\t</div>",
        after: "<div><lang>A</lang> <hw><pa>نجز</pa> (v.n.)</hw>, Performing (business), fulfilling (a promise); being present, in readiness; — <i>najaz</i> (v.n.) Being performed (a promise), being finished (a business or discourse); being at the end of a journey; perishing, being ruined; vanishing.</div>",
        reason: "Move definition out of headword; historical correction recorded in 03d1465d5176.",
    },
    Correction {
        id: 18269,
        field: "raw_html",
        before: "<div>&nbsp;&nbsp;\n\t\t\t<hw><pa>جزین</pa> <i>jazīdan</i></hw> , To change. \n\t\t\t</div>",
        after: "<div>&nbsp;&nbsp;\n\t\t\t<hw><pa>جزیدن</pa> <i>jazīdan</i></hw> , To change. \n\t\t\t</div>",
        reason: "Restore missing dal in jazīdan; https://github.com/theodore-s-beers/steingass-scraper/issues/3",
    },
    Correction {
        id: 39272,
        field: "raw_html",
        before: "<div>&nbsp;&nbsp;\n\t\t\t<lang>A</lang> <hw><pa>عاشتق</pa> <i>ʻāshiq</i></hw> , A lover, inamorato, sweetheart, mistress; amorous; transported with divine love; — <i>ʻāshiqi bī-chāra</i>, A species of ivy; — <i>ʻāshiqi ḵẖushq</i>, A faithless lover; — <i>ʻāshiqi dīwāna</i>, Frantic, mad, dying with love; — <i>ʻāshiqi sag-jān</i>, A worldling; — <i>ʻāshiq shudan</i>, To fall in love; — <i>ʻāshiqi majnūn (maftūn)</i>, Madly in love; — <i>ʻāshiq u maʻshūq</i>, Lover and beloved, a couple of lovers. \n\t\t\t</div>",
        after: "<div>&nbsp;&nbsp;\n\t\t\t<lang>A</lang> <hw><pa>عاشق</pa> <i>ʻāshiq</i></hw> , A lover, inamorato, sweetheart, mistress; amorous; transported with divine love; — <i>ʻāshiqi bī-chāra</i>, A species of ivy; — <i>ʻāshiqi ḵẖushq</i>, A faithless lover; — <i>ʻāshiqi dīwāna</i>, Frantic, mad, dying with love; — <i>ʻāshiqi sag-jān</i>, A worldling; — <i>ʻāshiq shudan</i>, To fall in love; — <i>ʻāshiqi majnūn (maftūn)</i>, Madly in love; — <i>ʻāshiq u maʻshūq</i>, Lover and beloved, a couple of lovers. \n\t\t\t</div>",
        reason: "Remove spurious ta in āshiq (printed pp. 829–830); reported by Assanali Iskakov, 2026-11-11. https://steingass.theobeers.com/page-img/0829.jpg",
    },
];

pub const FIELDS: &[Correction] = &[
    Correction {
        id: 83,
        field: "definitions",
        before: "Arrangement of the Arabic alphabet, according to the numeral value of the letters from one to a thousand, as follows:  \n<img src=\"/dictionaries/steingass/images/abjad_p0005.jpg\" style=\"width:50.0%\" data-border=\"0\" alt=\"Arabic alphabet according to numeral values\" />  \n*abjadi tajrīd nawishtan*, To abandon the world, to devote one's self wholly to God; — *abjad-ḵẖẉān*, One learning his alphabet.",
        after: "Arrangement of the Arabic alphabet, according to the numeral value of the letters from one to a thousand, as follows: [image removed]; — *abjadi tajrīd nawishtan*, To abandon the world, to devote one's self wholly to God; — *abjad-ḵẖẉān*, One learning his alphabet.",
        reason: "Preserve the manually shortened abjad definition with image removed; commit 15ba58999ec6. Expected conversion uses Pandoc 3.13.",
    },
    Correction {
        id: 39208,
        field: "headword_persian",
        before: "ع ‘",
        after: "ع",
        reason: "Separate the ayn letter from its transcription; preserved historical field edit (commit 98024188).",
    },
    Correction {
        id: 39208,
        field: "headword_latin",
        before: "N/A",
        after: "‘",
        reason: "Preserve the ayn transcription outside an italic element; historical field edit (commit 98024188).",
    },
];

module.exports = grammar({
  name: "lpi",

  extras: ($) => [/\s+/],

  rules: {
    document: ($) => seq(optional($.xmlDeclaration), repeat1($.element)),

    // Lazarus always writes the declaration, and it is the only processing
    // instruction the format uses.
    xmlDeclaration: ($) => seq("<?xml", repeat($.attribute), "?>"),

    // Open and close tags are separate rules so that a query can anchor an
    // indent or a bracket pair on one of them without naming the other.
    element: ($) =>
      choice(
        seq(field("open", $.startTag), repeat($._child), field("close", $.endTag)),
        $.selfClosingTag
      ),

    startTag: ($) => seq("<", field("name", $.name), repeat($.attribute), ">"),

    selfClosingTag: ($) =>
      seq("<", field("name", $.name), repeat($.attribute), "/>"),

    endTag: ($) => seq("</", $.name, ">"),

    // Hidden so that children sit directly under their parent element: a
    // visible wrapper node would put one more level between a query and the
    // tags it selects.
    _child: ($) => choice($.element, $.text),

    // The format stores every value in an attribute, so character data only
    // shows up in hand-edited files; it exists so that stray content does not
    // become a parse error.
    text: ($) => token(/[^<&]+/),

    attribute: ($) =>
      seq(field("key", $.name), "=", field("value", $.quotedString)),

    quotedString: ($) => seq('"', field("content", optional($.content)), '"'),

    // One token rather than a repeat of characters, so a query can capture the
    // whole value. Newlines are part of it: package descriptions and
    // `Conditionals` really do span several lines, and the entity alternative
    // keeps `&apos;` from ending the string.
    content: ($) => token(/[^"&]*(?:&[A-Za-z#][A-Za-z0-9]*;[^"&]*)*/),

    name: ($) => /[A-Za-z_][A-Za-z0-9_.:-]*/,
  },
});

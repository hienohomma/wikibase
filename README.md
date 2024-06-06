# Wikibase

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

[Wikipedia](https://en.wikipedia.org) being the public graphical database of the humankind, we might be able to read seldomly changing data correctly from there and store it as JSON, right?

Lets see.

Collects following data automatically:

- [Calling codes](output/calling_codes.json)  
- [Capital cities](output/capitals.json)  
- [Currencies](output/currencies.json)  
- [Flag emojis](output/emojis.json)  
- [Flags](output/flags.json) _dirs to flag images_  
- [Languages](output/languages.json)  
- [Regions - ISO 3166](output/regions.json)  
- [Sovereign states](output/sovereign_states.json)  
- [UN nations](output/un_nations.json)

One could also write all JSON above manually and be done with it, but chances for that data to ever be updated would be quite low.

[Output](output) directory contains all results of the latest successful execution of the program.  
If wikipedia doesn't change its page layout dramatically this generator should work (at least with minor updates) in the future as well.

## Description

Working principle is following:

1. Use [input map](input/countries.json) to identify countries in [UN member states](https://www.un.org/en/about-us/member-states) and [sovereign states](https://en.wikipedia.org/wiki/List_of_sovereign_states) list in wikipedia
2. Exclude countries that are not detected as UN members
3. Get ISO 3166 [regions](output/regions.json) and connect [sovereign states](output/sovereign_states.json) to  2 letter country codes
4. Use 2 letter ISO codes to identify regions in other lists such as [capital cities](output/capitals.json), [currencies](output/currencies.json), [flag emojis](output/emojis.json) and so on

### JSON

[UN member states](https://www.un.org/en/about-us/member-states) list was selected as the source of truth, output is primarly generated for countries present in this list.

However, [regions.json](output/regions.json) which is generated from `ISO 3166 country codes` holds references to non UN members as well.  
Autonomous regions of UN member states are included and used later on for filtering other data sets.

For example [Åland](https://en.wikipedia.org/wiki/%C3%85land) is included in [regions](output/regions.json) as it's autonomous region of [Finland](https://en.wikipedia.org/wiki/Finland)

``` json
"ax": {
    "name": "Åland",
    "state_name": "Åland",
    "un_member": false,
    "sovereignity": "fi",
    "iso_3166_1": {
      "a2": "AX",
      "a3": "ALA",
      "num": 248
    },
    "iso_3166_2": "ISO 3166-2:AX",
    "tld": [
      ".ax"
    ]
}
```

Now we have flag [emoji](output/emojis.json) 🇦🇽 of Åland

``` json
"ax": "🇦🇽",
```

And [calling code](output/calling_codes.json)

``` json
"ax": "358 (18)",
```

And Mariehamn is included in [capital cities](output/capitals.json) list

``` json
"ax": {
    "name": "Mariehamn",
    "endonyms": [
      "Maarianhamina"
    ]
}
```

### Flags

Program also downloads flags of sovereign states and runs transformations on them to generate some rounded versions of the flag:  
![Finland round, black frame](output/flags/fi/round_bl.png)
![Switzerland round, black frame](output/flags/ch/round_wh.png)
![Colombia round, yellow frame](output/flags/co/round_r.png)
![Brazil round, green frame](output/flags/br/round_g.png)

As you can see sizes might vary. Code only crops max sized square from the center of the source image.

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
- [Contributing](#contributing)
- [License](#license)
- [Contact](#contact)

## Installation

You need to [install rust](https://www.rust-lang.org/tools/install) and cargo to compile the binary.  
Only tested on linux, if binary compiles / program works on windows or mac that's completely unintentional.

## Usage

Clone this project and navigate to it's parent directory.  
Destroy `output` directory to have everything refetched from wikipedia.  
Execute binary and see if it teminates with exitcode 0, regenerating the `output` dir.

```bash
cd wikibase
rm -rf output
cargo run
```

## Contributing

Pull requests, reported issues, improvements in documentation etc. are always welcome.  
Try to behave while at it.

## License

This project is licensed under the [MIT License](https://opensource.org/licenses/MIT).

Content pulled from wikipedia might have different licences.  
Though, the type of data collected here should, in my humble opinion, be treated as [public information](https://en.wikipedia.org/wiki/Public_domain) to which nobody can really own the rights to.

## Contact

- Email: <opensource@hienohomma.fi>
- GitHub: [hienohomma](https://github.com/hienohomma)

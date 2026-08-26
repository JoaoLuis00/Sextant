## Question to ask when adding a derive?
| Derive        | Why you'll use it                                                              |
| ------------- | ------------------------------------------------------------------------------ |
| `Debug`       | Print values while debugging.                                                  |
| `Clone`       | Explicitly duplicate owned data.                                               |
| `PartialEq`   | Compare values and write tests.                                                |
| `Eq`          | Only if the type has true equality (no `f64`).                                 |
| `Hash`        | Use the type as a key in `HashMap`/`HashSet`.                                  |
| `Default`     | Create a sensible default value (I would avoid this for most domain entities). |
| `Serialize`   | Convert Rust types to JSON or other formats.                                   |
| `Deserialize` | Build Rust types from JSON or other formats.                                   |

#derive[(D1,D2,...)]
use crate::placeholders::Location;
use crate::Placeholders;
use http::{HeaderMap, HeaderName};
use miette::{miette, Result};
use rede_schema::body::FormDataValue;
use rede_schema::{Body, Request};
use std::collections::HashMap;

pub struct Renderer {
    placeholders: Placeholders,
    values_map: HashMap<String, String>,
}

macro_rules! replace_pointer {
    ($pointer:expr, $placeholder:expr, $value:expr) => {
        let new_value = $pointer.replace($placeholder, $value);
        *$pointer = new_value;
    };
}

impl Renderer {
    /// todo doc
    #[must_use]
    pub fn new(placeholders: Placeholders, values: &[(String, String)]) -> Self {
        let values_map = values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        Self {
            placeholders,
            values_map,
        }
    }

    /// todo doc
    ///
    /// # Errors
    ///
    /// todo
    ///
    /// # Panics
    ///
    /// todo -> if the request structure does not match the placeholders
    pub fn render(&self, request: Request) -> Result<Request> {
        let mut url = request.url;
        let mut headers = request.headers;
        let mut query_params = request.query_params;
        let mut body = request.body;

        for (key, locations) in self.placeholders.iter() {
            let val = self.values_map.get(key); // todo maybe this could be changed into a map
            if let Some(val) = val {
                let placeholder = format!("{{{{{key}}}}}");
                for location in locations {
                    match location {
                        Location::Url => url = url.replace(&placeholder, val),
                        Location::Headers(name) => {
                            render_headers(&mut headers, name, &placeholder, val)?;
                        }
                        Location::QueryParams(key) => {
                            if let Some((_, v)) = query_params.iter_mut().find(|(k, _)| k == key) {
                                replace_pointer!(v, &placeholder, val);
                            }
                        }
                        Location::BodyForm(k) => match &mut body {
                            Body::FormData(form) => {
                                render_form_data(form, k, &placeholder, val);
                            }
                            Body::XFormUrlEncoded(form) => {
                                render_form_urlencoded(form, k, &placeholder, val);
                            }
                            _ => panic!("unexpected body type"),
                        },
                        Location::Body => { /* todo */ }
                    }
                }
            }
        }

        Ok(Request {
            method: request.method,
            url,
            http_version: request.http_version,
            metadata: request.metadata,
            headers,
            query_params,
            variables: request.variables,
            body,
        })
    }
}

fn render_headers(
    header_map: &mut HeaderMap,
    header: &HeaderName,
    placeholder: &str,
    val: &str,
) -> Result<()> {
    if let Some(header_value) = header_map.get_mut(header) {
        let new_value = header_value.to_str().map_err(|_| {
            miette!("failed to convert header to string: {header} {header_value:?}")
        })?;
        let new_value = new_value.to_string().replace(placeholder, val);
        *header_value = new_value
            .parse()
            .map_err(|_| miette!("rendered header value is invalid: {header} {new_value}"))?;
    }
    Ok(())
}

fn render_form_data(
    form: &mut HashMap<String, FormDataValue>,
    key: &str,
    placeholder: &str,
    val: &str,
) {
    if let Some(FormDataValue::Text(v) | FormDataValue::File(v)) = form.get_mut(key) {
        replace_pointer!(v, placeholder, val);
    }
}

fn render_form_urlencoded(
    form: &mut HashMap<String, String>,
    key: &str,
    placeholder: &str,
    val: &str,
) {
    if let Some(v) = form.get_mut(key) {
        replace_pointer!(v, placeholder, val);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use http::{HeaderMap, Method, Version};
    use std::error::Error;

    #[test]
    fn render() -> std::result::Result<(), Box<dyn Error>> {
        // todo replace by generated placeholders
        let mut placeholders = Placeholders::default();
        placeholders.add_all(&Location::Url, vec!["id", "name"]);
        placeholders.add_all(
            &Location::Headers("Authorization".parse().unwrap()),
            vec!["token"],
        );
        placeholders.add_all(&Location::QueryParams("page".to_string()), vec!["page"]);
        placeholders.add_all(&Location::QueryParams("size".to_string()), vec!["size"]);
        placeholders.add_all(&Location::Body, vec!["id", "name"]);

        let values = vec![
            ("id".to_string(), "1".to_string()),
            ("name".to_string(), "test".to_string()),
            ("token".to_string(), "abc".to_string()),
            ("page".to_string(), "1".to_string()),
            ("size".to_string(), "10".to_string()),
        ];

        let renderer = Renderer::new(placeholders, &values);

        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers.insert("Authorization", "Bearer {{token}}".parse().unwrap());

        let mut query_params = Vec::new();
        query_params.push(("page".to_string(), "{{page}}".to_string()));
        query_params.push(("size".to_string(), "{{size}}".to_string()));

        let request = Request {
            method: Method::GET,
            url: "https://example.com/{{id}}/{{name}}/{{id}}".to_string(),
            http_version: Version::HTTP_11,
            metadata: HashMap::new(),
            headers,
            query_params,
            variables: HashMap::new(),
            body: rede_schema::Body::None,
        };

        let rendered = renderer.render(request).unwrap();

        assert_eq!(rendered.url, "https://example.com/1/test/1");
        assert_eq!(rendered.headers["Authorization"].to_str()?, "Bearer abc");
        assert_eq!(
            rendered.query_params,
            vec![
                ("page".to_string(), "1".to_string()),
                ("size".to_string(), "10".to_string()),
            ]
        );
        Ok(())
    }

    #[test]
    fn render_form_data() {
        let mut form = HashMap::new();
        form.insert(
            "name".to_string(),
            FormDataValue::Text("{{name}}".to_string()),
        );
        form.insert(
            "file".to_string(),
            FormDataValue::File("{{path}}/file".to_string()),
        );

        super::render_form_data(&mut form, "name", "{{name}}", "temp_file");
        super::render_form_data(&mut form, "file", "{{path}}", "/tmp");

        assert_eq!(form["name"], FormDataValue::Text("temp_file".to_string()));
        assert_eq!(form["file"], FormDataValue::File("/tmp/file".to_string()));
    }

    #[test]
    fn render_form_urlencoded() {
        let mut form = HashMap::new();
        form.insert("page".to_string(), "{{page}}".to_string());
        form.insert("order".to_string(), "{{field}}:asc".to_string());

        super::render_form_urlencoded(&mut form, "page", "{{page}}", "10");
        super::render_form_urlencoded(&mut form, "order", "{{field}}", "id");

        assert_eq!(form["page"], "10".to_string());
        assert_eq!(form["order"], "id:asc".to_string());
    }
}

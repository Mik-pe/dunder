# HTTP för verkliga klienter och servrar

HTTP-server och HTTP-klient levereras som Dunderpaket. Med en lokal kopia
av paketen kan ett projekt lägga till dem i sitt `dunder.toml`.

```toml
[dependencies]
http = { path = "../dunder/packages/http", capabilities = ["network", "unsafe"], native_libs = ["c"] }
http_client = { path = "../dunder/packages/http_client", capabilities = ["network", "unsafe"], native_libs = ["/usr/lib/libcurl.dylib", "libcurl.so.4"] }
```

Importera dem med `import http` och `import http_client`.
Nätverksåtkomst kräver `capability network` hos anroparen.
[HTTP-serverpaketet](../packages/http/) · [HTTP-klientpaketet](../packages/http_client/)

`http.read_request(&stream, max_body, timeout_ms)` läser ett komplett
HTTP/1.0- eller HTTP/1.1-anrop. Resultatet innehåller `ok`, `status`, `error`,
`method`, `target`, parallella `header_names`/`header_values` och binära `body`.
Headernamnen är normaliserade till gemener. `http.header(&request, name)`
hämtar ett värde med ett sådant namn, och `http.headers(&request, name)`
returnerar alla matchande värden i ordning.

En server som behöver välja gräns eller strömma efter att ha sett sökvägen
använder `http.read_head(&stream, timeout_ms)` först. Den konsumerar exakt
headerblocket och lämnar kroppens byte i socketen. Därefter kan servern
anropa `http.read_body(&stream, move head, max_body, timeout_ms)` eller
strömma kroppen till en annan gränskontrollerad konsument. `read_head`
accepterar en giltig 64-bitars Content-Length utan att allokera kroppen.
`read_body` och `read_request` har en explicit gräns på högst 64 MiB.

Stora uppladdningar använder `http.BodyReader::new(&head, max_bytes, timeout_ms)`.
Kontrollera `.ok()`, `.status()` och `.error()` innan någon lagringsyta skapas.
Efter autentisering och staging anropas `.read(&stream, &mut chunk, max_chunk)`
upprepade gånger. Varje anrop tömmer bufferten och returnerar
`BodyResult { ok, eof, count, status, error }`; positivt `count` följs av ett
separat anrop med `eof == true`. Buffertgränsen är 1..1 MiB och totalgränsen
är ett 64-bitars heltal. Dekodern samlar aldrig hela kroppen i minnet.
Första läsningen skickar eventuellt `100 Continue`, så behörighet och kvot
kan kontrolleras före detta svar. Callern kan kontrollera sin lease mellan
läsningar; en gemensam deadline gäller också chunked-data och trailers.

Parsern begränsar headers till 32 KiB och en rad till 8192 byte. Den avvisar
bland annat tvetydig Content-Length/Transfer-Encoding, dubblerade
kontrollheaders, ogiltiga CRLF och förbjudna trailers. Även egna tokenfält
på formen `X-…-Token` avvisas i trailers. Content-Length och
chunked-kroppar bevarar godtyckliga binära byte. `100 Continue` skickas först
efter att kroppens deklarerade gränser har kontrollerats. Läsning använder
icke-blockerande sockets och en monotont uppmätt deadline. `read_request`
delar samma totala tidsbudget mellan header och kropp; de separata anropen
har var sin uttryckliga budget. Anroparen äger och stänger socketen.

`http.write_response` skriver ett engångssvar med explicit innehållstyp och
egna headers med `Connection: close` och korrekta kroppslösa HEAD/204-svar.
`http.write_all` hanterar delskrivningar och skrivdeadline.

För strukturerad svarsuppbyggnad och persistent HTTP/1.1 finns byggaren
`http.Response` (`Response::new(status)`, `ok`, `text`, `html`, `json`,
`bytes`, `empty`, `redirect`, `.header`/`.with_header`, `.set_content_type`/`.with_content_type`,
`.set_body_text`/`.set_body_bytes`, `.set_keep_alive` och `.set_cookie`) tillsammans
med `connection_wants_close`, `connection_wants_keep_alive`, `should_keep_alive`,
`encode_response`, `send_response` och `graceful_close` (ordnad half-close-tömning).

För fleranvändarservrar erbjuder `packages/http` en multiplexerad HTTP/1.1
keep-alive-server via `http.serve(listener, config, cancel, &handler)` (samt
`http.serve_channel`) byggd på `std.net.PollSet`/`Waker`, `std.sync.Channel`
och en `task scope`-arbetarpool: en händelseloop multiplexar vilande
anslutningar och lämnar aktiva förfrågningar till arbetartrådar som implementerar
traiten `http.Handler`. Dessutom finns webbprimitiv utan inbyggd router:
`percent_encode` / `percent_decode` / `percent_decode_bytes`,
`form_encode_component` / `form_decode`, `target_path` / `target_query`,
`parse_query` / `parse_form_urlencoded` / `encode_form_urlencoded` / `param_get` / `param_get_all` (`FormParam`),
`Cookie` / `parse_cookies` / `cookie` / `SetCookie` / `SameSite`,
`BasicCredentials` / `parse_bearer` / `parse_basic` / `format_bearer` / `format_basic`
samt `escape_html`. Modulen innehåller fortsatt ingen router eller behörighetspolicy.
[HTTP- och webbprov](../tools/test_http_web.ddr)

`http_client.request(method, url, &names, &values, &body, timeout_ms, max_bytes)`
returnerar `Response { ok, status, transport_code, body, error }`. `ok` gäller transporten;
en HTTP 404 är ett transportmässigt lyckat svar med `status == 404`.
Anroparen väljer hur applikationsstatus ska hanteras. Svarskroppen är binär
och dess gräns kontrolleras innan callbacken lägger till fler byte.
`transport_code` är libcurl-koden (till exempel 7 för misslyckad anslutning,
28 för timeout och 42 för avbrott); lyckad HTTP-transport och lokala
valideringsfel har kod 0. Klassificera transportfel med denna kod, inte
meddelandets operativsystemsberoende text.

Klienten använder systemets libcurl, verifierar TLS-certifikat och värdnamn,
tillåter endast HTTP/HTTPS och följer inga omdirigeringar. En anslutning har
högst fem sekunder och hela anropet den valda deadlinen. En privat PKI kan
anges genom `request_with_ca(..., ca_bundle)` utan att stänga av TLS-kontroll.
Ingen modul loggar credentials. För samtidiga task-anrop krävs en libcurl
med trådsäker global initiering; en hårt begränsad DNS-uppslagning kräver
asynkron DNS i libcurl.

`request_monitored(..., monitor)` och
`request_monitored_with_ca(..., ca_bundle, monitor)` lägger till en
synkron `&mut FnMut() -> bool`-closure för avbrott och leaseförnyelse.
`false` avbryter även en anslutning som väntar på HTTP-headers eller
TLS-handshake, och closuren får inte lämna sin Dunder-tråd. Ett separat
HTTP-anrop för leaseförnyelse kan göras från callbacken. Svarens deklarerade
och faktiskt mottagna längd kontrolleras mot samma gräns innan någon större
buffert allokeras.

På macOS används systemets libcurl. Linux behöver `libcurl.so.4`.
Övervakade anrop använder `curl_multi_poll` (libcurl 7.66 eller senare)
och väntar högst 100 ms per pollning innan monitorn anropas igen.
Klientsviten använder OpenSSL för lokala TLS-fixturer.

```sh
/absolute/path/to/dunder run tools/test_http.ddr -- --compiler /absolute/path/to/dunder
/absolute/path/to/dunder run tools/test_http_client.ddr -- /absolute/path/to/dunder
```

Sviterna testar riktiga sockets, fragmentering, gränser, deadlines och
TLS-förtroende. Linux-korsbyggen ersätter inte körning på en Linux-värd.

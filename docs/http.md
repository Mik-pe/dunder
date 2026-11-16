# HTTP för verkliga klienter och servrar

`std.http` och `std.http_client` är separata moduler skrivna i Dunder. Snus
använder dem för sin publika HTTP-server och sin CLI. Nätverksåtkomst kräver
`capability network` hos anroparen.

`http.read_request(&stream, max_body, timeout_ms)` läser ett komplett
HTTP/1.0- eller HTTP/1.1-anrop. Resultatet innehåller `ok`, `status`, `error`,
`method`, `target`, parallella `header_names`/`header_values` och binära `body`.
Headernamnen är normaliserade till gemener. `http.header(&request, name)`
hämtar ett värde med ett sådant namn.

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
kontrollheaders, ogiltiga CRLF och förbjudna trailers. Content-Length och
chunked-kroppar bevarar godtyckliga binära byte. `100 Continue` skickas först
efter att kroppens deklarerade gränser har kontrollerats. Läsning använder
icke-blockerande sockets och en monotont uppmätt deadline. `read_request`
delar samma totala tidsbudget mellan header och kropp; de separata anropen
har var sin uttryckliga budget. Anroparen äger och stänger socketen.

`http.write_response` skriver en response med explicit innehållstyp och
egna headers. Den använder Connection: close och korrekta kroppslösa
HEAD/204-svar. `http.write_all` hanterar delskrivningar och skrivdeadline.
Modulen innehåller ingen router, trådpool eller behörighetspolicy.

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

`request_monitored(..., monitor, context)` och
`request_monitored_with_ca(..., ca_bundle, monitor, context)` lägger till en
synkron `fn(ptr) -> bool`-callback för avbrott och leaseförnyelse. Anropen är
`unsafe`: kontextpekaren måste leva under hela överföringen och callbacken
får inte lämna sin Dunder-tråd. `false` avbryter även en anslutning som väntar
på HTTP-headers eller TLS-handshake. Ett separat HTTP-anrop för leaseförnyelse
kan göras från callbacken. Svarens deklarerade och faktiskt mottagna längd
kontrolleras mot samma gräns innan någon större buffert allokeras.

På macOS används systemets libcurl; Linux behöver `libcurl.so.4`.
Verifieringen använder Python 3 och OpenSSL för lokala TLS-fixturer:

```sh
python3 tools/test-http.py --compiler /absolute/path/to/dunder
python3 tools/test-http-client.py /absolute/path/to/dunder
```

Sviterna testar riktiga sockets, fragmentering, gränser, deadlines och
TLS-förtroende. Linux-korsbyggen ersätter inte körning på en Linux-värd.

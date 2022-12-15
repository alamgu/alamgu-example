import { expect } from 'chai';
import { describe, it } from 'mocha';
import SpeculosTransport from '@ledgerhq/hw-transport-node-speculos';
import Axios from 'axios';
import Transport from "./common";
import { Common } from "hw-app-obsidian-common";
import * as blake2b from "blake2b";
import { instantiate, Nacl } from "js-nacl";

let ignoredScreens = [ "W e l c o m e", "Cancel", "Working...", "Exit", "Rust App 0.0.1"]

const API_PORT: number = 5005;

const BASE_URL: string = `http://127.0.0.1:${API_PORT}`;

let setAcceptAutomationRules = async function() {
    await Axios.post(BASE_URL + "/automation", {
      version: 1,
      rules: [
        ... ignoredScreens.map(txt => { return { "text": txt, "actions": [] } }),
        { "y": 16, "actions": [] },
        { "y": 31, "actions": [] },
        { "y": 46, "actions": [] },
        { "text": "Confirm", "actions": [ [ "button", 1, true ], [ "button", 2, true ], [ "button", 2, false ], [ "button", 1, false ] ]},
        { "actions": [ [ "button", 2, true ], [ "button", 2, false ] ]}
      ]
    });
}

let processPrompts = function(prompts: [any]) {
  let i = prompts.filter((a : any) => !ignoredScreens.includes(a["text"])); // .values();
  let header = "";
  let prompt = "";
  let rv = [];
  for (var ii in i) {
    let value = i[ii];
    if(value["y"] == 1) {
      if(value["text"] != header) {
        if(header || prompt) rv.push({ header, prompt });
        header = value["text"];
        prompt = "";
      }
    } else if(value["y"] == 16) {
      prompt += value["text"];
    } else if((value["y"] == 31)) {
      prompt += value["text"];
    } else if((value["y"] == 46)) {
      prompt += value["text"];
    } else {
      if(header || prompt) rv.push({ header, prompt });
      rv.push(value);
      header = "";
      prompt = "";
    }
  }
  if (header || prompt) rv.push({ header, prompt });
  return rv;
}

let fixActualPromptsForSPlus = function(prompts: any[]) {
  return prompts.map ( (value) => {
    if (value["text"]) {
      value["x"] = "<patched>";
    }
    return value;
  });
}

// HACK to workaround the OCR bug https://github.com/LedgerHQ/speculos/issues/204
let fixRefPromptsForSPlus = function(prompts: any[]) {
  return prompts.map ( (value) => {
    let fixF = (str: string) => {
      return str.replace(/S/g,"").replace(/I/g, "l");
    };
    if (value["header"]) {
      value["header"] = fixF(value["header"]);
      value["prompt"] = fixF(value["prompt"]);
    } else if (value["text"]) {
      value["text"] = fixF(value["text"]);
      value["x"] = "<patched>";
    }
    return value;
  });
}

let sendCommandAndAccept = async function(command : any, prompts : any) {
    await setAcceptAutomationRules();
    await Axios.delete(BASE_URL + "/events");

    let transport = await Transport.open(BASE_URL + "/apdu");
    let client = new Common(transport, "rust-app");
    client.sendChunks = client.sendWithBlocks; // Use Block protocol
    let err = null;

    try { await command(client); } catch(e) {
      err = e;
    }
    if(err) throw(err);

    let actual_prompts = processPrompts((await Axios.get(BASE_URL + "/events")).data["events"] as [any]);
    try {
      expect(actual_prompts).to.deep.equal(prompts);
    } catch(e) {
      try {
        expect(fixActualPromptsForSPlus(actual_prompts)).to.deep.equal(fixRefPromptsForSPlus(prompts));
      } catch (_) {
        // Throw the original error if there is a mismatch as it is generally more useful
        throw(e);
      }
    }
}

describe('basic tests', () => {

  afterEach( async function() {
    await Axios.post(BASE_URL + "/automation", {version: 1, rules: []});
    await Axios.delete(BASE_URL + "/events");
  });

  it('provides a public key', async () => {

    await sendCommandAndAccept(async (client : Common) => {
      let rv = await client.getPublicKey("0");
      expect(rv.publicKey).to.equal("0451ec84e33a3119486461a44240e906ff94bf40cf807b025b1ca43332b80dc9dbfeeeecf616eb461fbb56e3d03fa385545c2d280c3449a2013a404606da512b08");
      expect(Buffer.from(rv.address, 'hex').toString()).to.equal("51ec84e33a3119486461a44240e906ff94bf40cf807b025b1ca43332b80dc9dbfeeeecf616eb461fbb56e3d03fa385545c2d280c3449a2013a404606da512b08"); // TODO: stop this coming out in hex?!
      return;
    }, [ ]
    );
  });
});

let nacl : Nacl =null;

instantiate(n => { nacl=n; });

function testTransaction(path: string, txn: string, prompts: any[]) {
     return async () => {
       let sig = await sendCommandAndAccept(
         async (client : Common) => {

           let pubkey = (await client.getPublicKey(path)).publicKey;

           // We don't want the prompts from getPublicKey in our result
           await Axios.delete(BASE_URL + "/events");

           let sig = await client.signTransaction(path, Buffer.from(txn, "utf-8").toString("hex"));
	   expect(sig).to.deep.equal({
		   signature: "3044022040bedef9f383d0af30d25400c6cde4c87d3fa1f79da661c35202faa72509a288022012522fe2f5315ca41e2dc50d59fb9f9652503713399cab3681e0ecd257cef1c8"
	   });
	   // Best to do a real signature check:
           /* expect(sig.signature.length).to.equal(128);
           let hash = blake2b(32).update(Buffer.from(txn, "utf-8")).digest();
           let pass = nacl.crypto_sign_verify_detached(Buffer.from(sig.signature, 'hex'), hash, Buffer.from(pubkey, 'hex'));
           expect(pass).to.equal(true);*/
         }, prompts);
     }
}

 describe("Signing tests", function() {
   before( async function() {
     while(!nacl) await new Promise(r => setTimeout(r, 100));
   })

   it("can sign a transaction",
      testTransaction(
        "0",
        "AABBCCDD",
        [
          {
            "header": "Sign Transaction",
            "prompt": "Hash: 47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU",
          },
          {
            "header": "For Address",
            "prompt": "51ec84e33a3119486461a44240e906ff94bf40cf807b025b1ca43332b80dc9dbfeeeecf616eb461fbb56e3d03fa385545c2d280c3449a2013a404606da512b08"
          },
          {
            "text": "Confirm",
            "x": 43,
            "y": 11,
          }
        ]
      ));
 });

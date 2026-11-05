// Synthetic Screen Compiler fixture: long text (contract v16). A header bar
// with a way back and a title, over an article the application sets at run
// time - paragraphs of any length up to 12 KiB, wrapped onto as many lines
// as they need - in a scroll view. Written for Screen Compiler's tests; not
// taken from any product.
import { Button, Column, HeaderBar, Screen, ScrollView, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const ARTICLE_BYTES = 12288;

type ArticleProps = {
  article: string;
  onBack: () => void;
};

export function Article(props: ArticleProps) {
  return (
    <Screen>
      <Column gap={GAP}>
        <HeaderBar gap={GAP} padding={EDGE}>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Text role="heading">The river path</Text>
        </HeaderBar>
        <ScrollView>
          <Column padding={EDGE}>
            <Text maxLength={ARTICLE_BYTES}>{props.article}</Text>
          </Column>
        </ScrollView>
      </Column>
    </Screen>
  );
}
